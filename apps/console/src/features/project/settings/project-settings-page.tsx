import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import {
  AlertDialog,
  AppFrame,
  Badge,
  Button,
  DialogScope,
  Input,
  InlineCopyText,
  NumberInput,
  SettingsGroup,
  SettingsRow,
  SettingsSection,
  Skeleton,
  Switch,
  Text,
} from "@logicplanes/kaeso-ui";
import { toast } from "~/shell/toast";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import type { ProjectLogSettings } from "~/api/generated/types";
import { orgQuery } from "~/api/queries/orgs";
import { projectListKey, projectQuery, projectViewKey } from "~/api/queries/projects";
import { useApiMutation } from "~/auth/mutation";
import { PictureControl, toPictureUpload } from "~/kaeso/picture";
import { ProjectHeader } from "~/shell/project-header";
import { useProjectRights } from "~/auth/rights";
import { useNeedsRight } from "~/auth/needs-right";
import { t } from "~/i18n";
import { common } from "~/i18n/common";

const logSettingsQuery = (org: string, project: string) => ({
  queryKey: ["orgs", org, "projects", project, "logs", "settings"] as const,
  queryFn: async (): Promise<ProjectLogSettings> =>
    data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/logs/settings", { params: { path: { org, project } } })),
});

export function ProjectSettingsPage({ org, project }: { org: string; project: string }) {
  const rights = useProjectRights(org, project);
  const view = useQuery(projectQuery(org, project));
  const p = view.data;
  return (
    <>
      <ProjectHeader org={org} project={project} page={common.settings()} path="settings" />
      <AppFrame.Body padding="page">
        <div className="flex w-full flex-col lg:mx-auto lg:max-w-720">
          {!p ? (
            <Skeleton className="h-full w-full" />
          ) : (
            <>
              <General
                org={org}
                project={project}
                name={p.name}
                src={p.avatar_url}
                archived={!!p.archived_at}
                id={p.id}
                createdAt={p.created_at}
                isDefault={p.is_default}
                canUpdate={rights.has("project.update")}
              />
              {rights.has("security.key_policy.project.manage") && (
                <KeyLifetime org={org} project={project} days={p.max_key_lifetime_days ?? null} archived={!!p.archived_at} />
              )}
              <ContentLogging org={org} project={project} canChange={rights.has("logs.content.manage")} archived={!!p.archived_at} />
              {p.archived_at ? (
                <ArchivedOn date={p.archived_at} />
              ) : (
                rights.has("project.archive") && <Archive org={org} project={project} name={p.name} isDefault={p.is_default} />
              )}
            </>
          )}
        </div>
      </AppFrame.Body>
    </>
  );
}

function General({
  org,
  project,
  name,
  src,
  archived,
  id,
  createdAt,
  isDefault,
  canUpdate,
}: {
  org: string;
  project: string;
  name: string;
  src: string | null | undefined;
  archived: boolean;
  id: string;
  createdAt: string;
  isDefault: boolean;
  canUpdate: boolean;
}) {
  const needs = useNeedsRight(org, project);
  const [draft, setDraft] = useState(name);
  const rename = useApiMutation({
    mutationFn: async (next: string) =>
      data(await api.PATCH("/platform/v1/orgs/{org}/projects/{project}", { params: { path: { org, project } }, body: { name: next } })),
    invalidates: [projectListKey(org), projectViewKey(org, project), ["me", "memberships"], ["session"]],
    onSuccess: () => toast.success(t("project.settings.renamed", "Project renamed")),
  });
  const next = draft.trim();
  const locked = !canUpdate ? needs("project.update") : archived ? archivedReadOnly() : undefined;
  const reason = locked ?? (!next ? t("org.general.enterName", "Enter a name") : next === name ? t("org.noChange", "No change to save") : undefined);
  return (
    <SettingsSection title={common.project()}>
      <SettingsGroup>
        <PictureRow
          org={org}
          project={project}
          name={name}
          src={src}
          disabledReason={!canUpdate ? needs("project.update") : archived ? archivedReadOnly() : undefined}
        />
        <SettingsRow
          title={common.name()}
          description={t("project.settings.nameHint", "Shown in the sidebar and in request logs.")}
          control={
            <div className="flex min-w-0 items-center gap-8">
              <div className="w-240 min-w-0">
                <Input
                  aria-label={t("project.settings.projectName", "Project name")}
                  size="sm"
                  value={draft}
                  maxLength={100}
                  disabled={!canUpdate || archived}
                  onChange={(e) => setDraft(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && !reason && !rename.isPending && rename.mutate(next)}
                />
              </div>
              <Button
                size="sm"
                disabled={!!reason}
                {...(reason && !locked ? { disabledReason: reason } : {})}
                loading={rename.isPending}
                onClick={() => rename.mutate(next)}
              >
                {common.save()}
              </Button>
            </div>
          }
        />
        <SettingsRow
          title={t("project.settings.id", "Project id")}
          description={t("project.settings.idHint", "Used by the Platform API.")}
          control={<InlineCopyText value={id} label={t("project.settings.copyId", "Copy project id")} />}
        />
        <SettingsRow title={common.created()} control={<Text tone="secondary">{formatDate(createdAt)}</Text>} />
        {isDefault && (
          <SettingsRow
            title={t("project.settings.defaultProject", "Default project")}
            description={t("project.settings.defaultHint", "New organization members join it; it can't be archived.")}
            control={<Badge>{t("project.settings.default", "Default")}</Badge>}
          />
        )}
      </SettingsGroup>
    </SettingsSection>
  );
}

function PictureRow({
  org,
  project,
  name,
  src,
  disabledReason,
}: {
  org: string;
  project: string;
  name: string;
  src: string | null | undefined;
  disabledReason: string | undefined;
}) {
  const path = { params: { path: { org, project } } };
  const invalidates = [projectListKey(org), projectViewKey(org, project), ["me", "memberships"], ["orgs", org, "credentials"]];
  const set = useApiMutation({
    mutationFn: async (file: File) => data(await api.PUT("/platform/v1/orgs/{org}/projects/{project}/avatar", { ...path, body: await toPictureUpload(file) })),
    invalidates,
    onSuccess: () => toast.success(t("project.settings.pictureSaved", "Picture saved")),
  });
  const remove = useApiMutation({
    mutationFn: async () => data(await api.DELETE("/platform/v1/orgs/{org}/projects/{project}/avatar", path)),
    invalidates,
    onSuccess: () => toast(t("project.settings.pictureRemoved", "Picture removed")),
  });
  return (
    <SettingsRow
      title={t("project.settings.picture", "Picture")}
      description={t("project.settings.pictureHint", "Shown beside the project's name, cropped to a square.")}
      control={
        <PictureControl
          name={name}
          src={src}
          kind="org"
          onPick={(f) => set.mutate(f)}
          onRemove={() => remove.mutate()}
          busy={set.isPending || remove.isPending}
          disabledReason={disabledReason}
        />
      }
    />
  );
}

function KeyLifetime({ org, project, days, archived }: { org: string; project: string; days: number | null; archived: boolean }) {
  const orgView = useQuery(orgQuery(org));
  const orgMax = orgView.data?.max_key_lifetime_days ?? null;
  const [draft, setDraft] = useState(days === null ? "" : String(days));
  const save = useApiMutation({
    mutationFn: async (next: number | null) =>
      data(
        await api.PUT("/platform/v1/orgs/{org}/projects/{project}/key-policy", { params: { path: { org, project } }, body: { max_key_lifetime_days: next } }),
      ),
    invalidates: [projectViewKey(org, project)],
    onSuccess: () => toast.success(t("project.settings.lifetimeSaved", "Key lifetime saved")),
  });
  const inherit = days === null;
  const value = Number(draft);
  const problem =
    draft && (!Number.isInteger(value) || value < 1)
      ? t("project.settings.wholeDays", "Enter whole days, at least 1.")
      : orgMax !== null && value > orgMax
        ? t("project.settings.orgAllowsAtMost", "The organization allows at most {days, plural, one {# day} other {# days}}.", { days: orgMax })
        : null;
  return (
    <SettingsSection
      title={t("project.settings.keyLifetime", "Key lifetime")}
      description={t("project.settings.keyLifetimeHint", "The longest a new key here may live; existing keys keep theirs.")}
    >
      <SettingsGroup>
        <SettingsRow
          title={t("project.settings.useOrgLimit", "Use the organization's limit")}
          description={
            orgMax === null
              ? t("project.settings.orgNoLimit", "The organization sets no limit.")
              : t("project.settings.atMost", "At most {days, plural, one {# day} other {# days}}.", { days: orgMax })
          }
          control={
            <Switch
              aria-label={t("project.settings.useOrgLimit", "Use the organization's limit")}
              checked={inherit}
              disabled={archived}
              {...(archived ? { disabledReason: archivedReadOnly() } : {})}
              onCheckedChange={(on) => {
                const next = on ? null : (orgMax ?? 90);
                if (next !== null) setDraft(String(next));
                save.mutate(next);
              }}
            />
          }
        />
        {!inherit && (
          <SettingsRow
            title={t("project.settings.maxLifetime", "Maximum lifetime")}
            description={t("project.settings.maxLifetimeHint", "Keys created here can't expire later than this.")}
            control={
              <div className="flex items-center gap-8">
                <div className="w-160">
                  <NumberInput
                    aria-label={t("project.settings.maxKeyLifetime", "Maximum key lifetime")}
                    size="sm"
                    integer
                    value={draft}
                    onValueChange={setDraft}
                    disabled={archived}
                    min="1"
                    step="1"
                    unit={t("project.settings.days", "days")}
                  />
                </div>
                <Button
                  size="sm"
                  disabled={archived || !draft || !!problem || value === days}
                  {...(archived ? { disabledReason: archivedReadOnly() } : problem ? { disabledReason: problem } : {})}
                  loading={save.isPending}
                  onClick={() => save.mutate(value)}
                >
                  {common.save()}
                </Button>
              </div>
            }
          />
        )}
      </SettingsGroup>
    </SettingsSection>
  );
}

function ContentLogging({ org, project, canChange, archived }: { org: string; project: string; canChange: boolean; archived: boolean }) {
  const needs = useNeedsRight(org, project);
  const settings = useQuery(logSettingsQuery(org, project));
  const toggle = useApiMutation({
    mutationFn: async (on: boolean) =>
      data(await api.PATCH("/platform/v1/orgs/{org}/projects/{project}/logs/settings", { params: { path: { org, project } }, body: { content_logging: on } })),
    invalidates: [projectViewKey(org, project), logSettingsQuery(org, project).queryKey],
    onSuccess: (s) =>
      toast.success(
        s.content_logging ? t("project.settings.loggingOn", "Content logging turned on") : t("project.settings.loggingOff", "Content logging turned off"),
      ),
  });
  const s = settings.data;
  const orgOff = !!s && s.content_logging && !s.effective_content_logging;
  return (
    <SettingsSection
      title={t("project.settings.contentLogging", "Content logging")}
      description={t("project.settings.contentLoggingHint", "Whether request and reply bodies are kept with each request log entry.")}
    >
      <SettingsGroup>
        <SettingsRow
          title={t("project.settings.logContent", "Log request content")}
          description={
            orgOff
              ? t("project.settings.loggingOrgOff", "Off for the organization; it can't be turned on here.")
              : s?.effective_content_logging
                ? t("project.settings.loggingKept", "Bodies are kept for the organization's content retention.")
                : t("project.settings.loggingMetadata", "Only metadata is kept: timing, tokens, cost and errors.")
          }
          control={
            <Switch
              aria-label={t("project.settings.logContent", "Log request content")}
              checked={!!s?.effective_content_logging}
              disabled={!s || orgOff || !canChange || archived}
              {...(!canChange ? { disabledReason: needs("logs.content.manage") } : archived ? { disabledReason: archivedReadOnly() } : {})}
              onCheckedChange={(on) => toggle.mutate(on)}
            />
          }
        />
      </SettingsGroup>
    </SettingsSection>
  );
}

const archivedReadOnly = () => t("project.settings.archivedReadOnly", "An archived project can't change");

function ArchivedOn({ date }: { date: string }) {
  return (
    <SettingsSection title={common.archive()}>
      <SettingsGroup>
        <SettingsRow
          title={t("project.settings.archivedTitle", "Archived")}
          description={t("project.settings.archivedHint", "Its keys and service accounts went with it; history is kept.")}
          control={<Text tone="secondary">{formatDate(date)}</Text>}
        />
      </SettingsGroup>
    </SettingsSection>
  );
}

function Archive({ org, project, name, isDefault }: { org: string; project: string; name: string; isDefault: boolean }) {
  const [open, setOpen] = useState(false);
  const button = (
    <Button
      variant="danger-secondary"
      size="sm"
      disabled={isDefault}
      {...(isDefault ? { disabledReason: t("project.settings.defaultCantArchive", "Make another project the default to archive this one") } : {})}
      onClick={() => setOpen(true)}
    >
      {t("project.settings.archiveProject", "Archive project")}
    </Button>
  );
  return (
    <SettingsSection title={common.archive()}>
      <SettingsGroup>
        <SettingsRow
          danger
          title={t("project.settings.archiveProject", "Archive project")}
          description={t("project.settings.archiveHint", "Its keys and service accounts go with it; history is kept.")}
          control={button}
        />
      </SettingsGroup>
      <DialogScope open={open}>
        <ArchiveDialog org={org} project={project} name={name} open={open} onOpenChange={setOpen} />
      </DialogScope>
    </SettingsSection>
  );
}

function ArchiveDialog({
  org,
  project,
  name,
  open,
  onOpenChange,
}: {
  org: string;
  project: string;
  name: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const navigate = useNavigate();
  const archive = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/orgs/{org}/projects/{project}/archive", { params: { path: { org, project } } })),
    invalidates: [projectListKey(org), ["orgs", org, "projects", project], ["me", "memberships"], ["session"]],
    onSuccess: () => {
      toast(t("project.settings.archived", "{name} archived", { name }));
      void navigate({ to: "/orgs/$org", params: { org } });
    },
  });
  return (
    <AlertDialog
      open={open}
      onOpenChange={onOpenChange}
      tone="danger"
      title={t("project.settings.archiveTitle", "Archive {name}?", { name })}
      description={t("project.settings.archiveConfirmHint", "Its keys stop authenticating with the gateway; archiving is permanent, and history is kept.")}
      confirmLabel={t("project.settings.archiveProject", "Archive project")}
      confirmText={name}
      loading={archive.isPending}
      onConfirm={() => archive.mutateAsync()}
    />
  );
}
