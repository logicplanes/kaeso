import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  AlertDialog,
  BarChart,
  Banner,
  Button,
  CodeBlock,
  DialogScope,
  Empty,
  Field,
  Input,
  KeyValue,
  SecretReveal,
  Section,
  Select,
  SettingsGroup,
  SettingsRow,
  Sheet,
  Skeleton,
  SourceBadge,
  Status,
  StepDialog,
  Switch,
  Text,
} from "@logicplanes/kaeso-ui";
import { formatDate, formatRelative } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import { isApiError } from "~/api/errors";
import { fieldError } from "~/api/field-error";
import type { ExternalSourceView, SourceUpdate } from "~/api/generated/types";
import { serviceAccountsQuery } from "~/api/queries/members";
import { sourceQuery } from "~/api/queries/sources";
import { useApiMutation } from "~/auth/mutation";
import { useNeedsRight } from "~/auth/needs-right";
import { useProjectRights } from "~/auth/rights";
import { useSession } from "~/auth/session";
import { compareNames } from "~/format/collate";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { SheetLinks } from "~/kaeso/sheet-links";
import { dayLabel } from "~/format/period";
import { toast } from "~/shell/toast";
import { usePartyName } from "../keys/party";
import { eventsByDay, kindName, refusalText, statusLabel, statusTone, totals } from "./labels";
import { KEY_PLACEHOLDER, setupSnippets } from "./setup";

const sourcesKey = (org: string, project: string) => ["orgs", org, "projects", project, "sources"];

export function SourceSheet({ org, project, sourceId, onClose }: { org: string; project: string; sourceId: string | undefined; onClose: () => void }) {
  const source = useQuery({ ...sourceQuery(org, project, sourceId ?? ""), enabled: !!sourceId });
  return (
    <Sheet open={!!sourceId} onOpenChange={(o) => !o && onClose()} modal={false} size="md" title={source.data?.name ?? t("project.sources.one", "Source")}>
      {sourceId && <SourceBody key={sourceId} org={org} project={project} sourceId={sourceId} onClose={onClose} />}
    </Sheet>
  );
}

function SourceBody({ org, project, sourceId, onClose }: { org: string; project: string; sourceId: string; onClose: () => void }) {
  const needs = useNeedsRight(org, project);
  const rights = useProjectRights(org, project);
  const source = useQuery(sourceQuery(org, project, sourceId));
  const accounts = useQuery({ ...serviceAccountsQuery(org, project, true), enabled: rights.has("service_accounts.view") });
  const partyName = usePartyName(org, project);
  const session = useSession();
  const [name, setName] = useState<string | null>(null);
  const [rotating, setRotating] = useState(false);
  const [archiving, setArchiving] = useState(false);
  const update = useApiMutation({
    mutationFn: async (body: SourceUpdate) =>
      data(
        await api.PATCH("/platform/v1/orgs/{org}/projects/{project}/sources/{source}", {
          params: { path: { org, project, source: sourceId } },
          body,
        }),
      ),
    invalidates: [sourcesKey(org, project)],
    onSuccess: (s, body) => {
      if (body.name) setName(null);
      toast.success(
        body.name
          ? t("project.sources.nameSaved", "Name saved")
          : body.disabled !== undefined && body.disabled !== null
            ? s.disabled_at
              ? t("project.sources.disabledToast", "Source disabled")
              : t("project.sources.enabledToast", "Source enabled")
            : t("project.sources.saved", "Source saved"),
      );
    },
  });

  if (source.isPending) return <Skeleton className="w-full" />;
  if (source.isError) {
    return (
      <Empty
        title={
          isApiError(source.error) && source.error.type === "not_found"
            ? t("project.sources.notFound", "This source doesn't exist or was removed.")
            : t("project.sources.loadFailed", "Couldn't load this source.")
        }
        actions={<Button onClick={onClose}>{t("project.sources.back", "Back to sources")}</Button>}
      />
    );
  }
  const s = source.data;
  const archived = !!s.archived_at;
  const canManage = rights.has("sources.manage") && !archived;
  const readOnly = archived ? t("project.sources.archivedReadOnly", "Archived sources can't change") : canManage ? undefined : needs("sources.manage");
  const sum = totals(s);
  const days = eventsByDay(s, new Date().toISOString().slice(0, 10));
  const account = s.service_account_id ? accounts.data?.find((a) => a.id === s.service_account_id) : undefined;
  const me = session.user.id;
  const owners = [
    ...(s.user_id && s.user_id !== me ? [{ value: s.user_id, label: partyName({ kind: "user", id: s.user_id }) }] : []),
    { value: me, label: t("project.sources.countsForMe", "You") },
    ...[...(accounts.data ?? [])]
      .filter((a) => !a.archived_at || a.id === s.service_account_id)
      .sort((a, b) => compareNames(a.name, b.name))
      .map((a) => ({ value: a.id, label: t("project.sources.countsForAccount", "{name} (service account)", { name: a.name }) })),
    ...(!s.user_id && !s.service_account_id ? [{ value: "", label: t("project.sources.countsForNobody", "Nobody") }] : []),
  ];
  const countsFor = s.service_account_id
    ? t("project.sources.countsForAccount", "{name} (service account)", { name: account?.name ?? s.service_account_id })
    : s.user_id
      ? partyName({ kind: "user", id: s.user_id })
      : t("project.sources.countsForNobody", "Nobody");

  return (
    <div className="flex flex-col gap-24">
      {archived && s.archived_at && (
        <Banner tone="neutral">
          {t("project.sources.archivedOn", "Archived {date}. Its key no longer works; its requests stay.", { date: formatDate(s.archived_at) })}
        </Banner>
      )}
      {!archived && s.disabled_at && (
        <Banner tone="warning">{t("project.sources.disabledBanner", "Disabled: Kaeso refuses what it sends until you enable it again.")}</Banner>
      )}
      {name !== null ? (
        <Field label={common.name()} error={fieldError(update.error, "name")}>
          <div className="flex items-center gap-8">
            <Input
              className="flex-1"
              value={name}
              onChange={(e) => {
                setName(e.target.value);
                if (update.isError) update.reset();
              }}
              autoFocus
              maxLength={100}
            />
            <Button
              variant="primary"
              disabled={!name.trim() || name.trim() === s.name}
              {...(!name.trim() || name.trim() === s.name ? { disabledReason: t("project.sources.renameFirst", "Type a new name first.") } : {})}
              loading={update.isPending && update.variables?.name !== undefined}
              onClick={() => update.mutate({ name: name.trim() })}
            >
              {common.save()}
            </Button>
            <Button onClick={() => setName(null)}>{common.cancel()}</Button>
          </div>
        </Field>
      ) : (
        <KeyValue
          items={[
            {
              label: common.name(),
              value: (
                <span className="flex flex-wrap items-center gap-x-8 gap-y-4">
                  <span className="min-w-0 wrap-break-word">{s.name}</span>
                  {!archived && (
                    <Button variant="ghost" size="xs" disabled={!!readOnly} {...(readOnly ? { disabledReason: readOnly } : {})} onClick={() => setName(s.name)}>
                      {common.rename()}
                    </Button>
                  )}
                </span>
              ),
            },
            { label: t("project.sources.kindLabel", "Kind of source"), value: <SourceBadge kind={s.kind} size="sm" withName /> },
            {
              label: common.status(),
              value: (
                <span className="flex flex-wrap items-center gap-x-8 gap-y-4">
                  <Status tone={statusTone(s.status)} size="sm">
                    {statusLabel(s.status)}
                  </Status>
                  <Text variant="small" tone="secondary">
                    {s.last_event_at
                      ? t("project.sources.lastEventAt", "Last event {when}", { when: formatRelative(s.last_event_at) })
                      : t("project.sources.noEvent", "No event yet")}
                  </Text>
                </span>
              ),
              wrap: true,
            },
            { label: t("project.sources.key", "Key"), value: s.key, mono: true },
            { label: t("project.sources.countsFor", "Counts for"), value: countsFor, wrap: true },
            {
              label: common.created(),
              value: s.created_by
                ? t("project.sources.createdOnBy", "{date} by {name}", { date: formatDate(s.created_at), name: partyName({ kind: "user", id: s.created_by }) })
                : formatDate(s.created_at),
              wrap: true,
            },
            { label: t("project.sources.id", "Id"), value: s.id, mono: true, copy: s.id },
          ]}
        />
      )}
      <SheetLinks org={org} project={project} requests={{ source_id: s.id }} target={s.id} />
      {s.last_error && s.last_error_at && (
        <Banner tone="warning" title={t("project.sources.lastErrorAt", "Refused {when}", { when: formatRelative(s.last_error_at) })}>
          {refusalText(s.last_error)}
        </Banner>
      )}
      <Section
        title={t("project.sources.activity", "Activity")}
        description={t("project.sources.activityHint", "What it reported each day, UTC; duplicates and refusals aren't logged.")}
        spacing="none"
      >
        <div className="flex flex-col gap-12">
          <BarChart
            aria-label={t("project.sources.activityChart", "Requests and tool calls a day")}
            categories={days.map((d) => dayLabel(d.day))}
            dates={days.map((d) => d.day)}
            series={[
              {
                id: "requests",
                label: t("project.sources.requests", "Requests"),
                values: days.map((d) => s.activity.find((a) => a.day === d.day)?.requests ?? 0),
              },
              {
                id: "tool_calls",
                label: t("project.sources.toolCalls", "Tool calls"),
                values: days.map((d) => s.activity.find((a) => a.day === d.day)?.tool_calls ?? 0),
              },
            ]}
            layout="stacked"
            height={160}
            partial={{ last: true, ongoing: true }}
            emptyText={t("project.sources.noActivity", "Nothing reported in the last 14 days")}
          />
          <Text variant="small" tone="secondary" tabular>
            {t(
              "project.sources.activitySum",
              "{duplicates, plural, one {# duplicate} other {# duplicates}} left out, {refused, plural, one {# refused} other {# refused}}.",
              {
                duplicates: sum.duplicates,
                refused: sum.refused,
              },
            )}
          </Text>
        </div>
      </Section>
      <Section
        title={t("project.sources.setup", "Setup")}
        description={t("project.sources.setupHint", "The key was shown once; replace the placeholder, or give the source a new key.")}
        spacing="none"
      >
        <CodeBlock tabs={setupSnippets({ kind: s.kind, ingestUrl: s.ingest_url, secret: KEY_PLACEHOLDER, content: s.content })} wrap="phone" />
      </Section>
      {!archived && (
        <Section title={t("project.sources.settings", "Settings")} spacing="none">
          <SettingsGroup>
            <SettingsRow
              title={t("project.sources.countsFor", "Counts for")}
              description={t("project.sources.countsForRow", "Whom requests count for when no member matches.")}
              control={
                <Select
                  aria-label={t("project.sources.countsFor", "Counts for")}
                  options={owners}
                  value={s.service_account_id ?? s.user_id ?? ""}
                  disabled={!!readOnly}
                  onValueChange={(v: string) => update.mutate(v.startsWith("sa_") ? { service_account_id: v } : { user_id: v })}
                />
              }
            />
            <SettingsRow
              title={t("project.sources.matchMembers", "Match members by email")}
              description={t("project.sources.matchMembersRow", "An event with a member's email counts for them.")}
              control={
                <Switch
                  aria-label={t("project.sources.matchMembers", "Match members by email")}
                  checked={s.match_members}
                  disabled={!!readOnly}
                  {...(readOnly ? { disabledReason: readOnly } : {})}
                  loading={update.isPending && update.variables?.match_members !== undefined}
                  onCheckedChange={(v) => update.mutate({ match_members: v })}
                />
              }
            />
            <SettingsRow
              title={t("project.sources.content", "Keep prompts and replies")}
              description={t("project.sources.contentRow", "Only where the project keeps content too.")}
              control={
                <Switch
                  aria-label={t("project.sources.content", "Keep prompts and replies")}
                  checked={s.content}
                  disabled={!!readOnly}
                  {...(readOnly ? { disabledReason: readOnly } : {})}
                  loading={update.isPending && update.variables?.content !== undefined}
                  onCheckedChange={(v) => update.mutate({ content: v })}
                />
              }
            />
            <SettingsRow
              title={t("project.sources.enabled", "Accept its events")}
              description={t("project.sources.enabledRow", "Off refuses what it sends until turned on again.")}
              control={
                <Switch
                  aria-label={t("project.sources.enabled", "Accept its events")}
                  checked={!s.disabled_at}
                  disabled={!!readOnly}
                  {...(readOnly ? { disabledReason: readOnly } : {})}
                  loading={update.isPending && update.variables?.disabled !== undefined}
                  onCheckedChange={(v) => update.mutate({ disabled: !v })}
                />
              }
            />
          </SettingsGroup>
        </Section>
      )}
      {!archived && (
        <Section title={t("project.sources.keyAndArchive", "Key and archive")} spacing="none">
          <div className="flex flex-col items-start gap-12">
            <Text tone="secondary">
              {t("project.sources.rotateHint", "A new key stops the old one at once; archiving is permanent and keeps its history.")}
            </Text>
            <div className="flex flex-wrap gap-8">
              <Button size="sm" disabled={!!readOnly} {...(readOnly ? { disabledReason: readOnly } : {})} onClick={() => setRotating(true)}>
                {t("project.sources.rotate", "Give it a new key")}
              </Button>
              <Button
                variant="danger-secondary"
                size="sm"
                disabled={!!readOnly}
                {...(readOnly ? { disabledReason: readOnly } : {})}
                onClick={() => setArchiving(true)}
              >
                {t("project.sources.archive", "Archive source")}
              </Button>
            </div>
          </div>
        </Section>
      )}
      {rotating && <RotateDialog org={org} project={project} source={s} onClose={() => setRotating(false)} />}
      <DialogScope open={archiving}>
        <ArchiveDialog org={org} project={project} source={s} open={archiving} onOpenChange={setArchiving} onArchived={onClose} />
      </DialogScope>
      <Text variant="small" tone="tertiary">
        {t("project.sources.kindFooter", "Reports from {kind}; Kaeso is never in its requests' path, so limits alert but can't stop them.", {
          kind: kindName(s.kind),
        })}
      </Text>
    </div>
  );
}

function RotateDialog({ org, project, source, onClose }: { org: string; project: string; source: ExternalSourceView; onClose: () => void }) {
  const [secret, setSecret] = useState<string | null>(null);
  const rotate = useApiMutation({
    mutationFn: async () =>
      data(
        await api.POST("/platform/v1/orgs/{org}/projects/{project}/sources/{source}/rotate", {
          params: { path: { org, project, source: source.id } },
        }),
      ),
    invalidates: [sourcesKey(org, project)],
    onSuccess: (c) => setSecret(c.key),
  });
  return (
    <StepDialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t("project.sources.rotateTitle", "Give {name} a new key?", { name: source.name })}
      submitLabel={t("project.sources.rotate", "Give it a new key")}
      submitting={rotate.isPending}
      onSubmit={() => rotate.mutate()}
      result={
        secret
          ? {
              size: "lg",
              title: t("project.sources.rotated", "New key for {name}", { name: source.name }),
              description: t("project.sources.rotatedHint", "Copy it now, it's shown once; the old key stopped working, so update the client."),
              footer: <Button onClick={onClose}>{common.finish()}</Button>,
              content: (
                <SecretReveal
                  secret={secret}
                  name={source.name}
                  baseUrl={window.location.origin}
                  snippets={setupSnippets({ kind: source.kind, ingestUrl: source.ingest_url, secret, content: source.content })}
                />
              ),
            }
          : undefined
      }
      steps={[
        {
          id: "confirm",
          title: t("project.sources.rotateConfirm", "Replace the key"),
          description: t("project.sources.rotateConfirmHint", "The old key stops at once; events the client sends with it are refused until you update it."),
          content: <Text tone="secondary">{t("project.sources.rotateNote", "Its logged requests and settings stay as they are.")}</Text>,
        },
      ]}
    />
  );
}

function ArchiveDialog({
  org,
  project,
  source,
  open,
  onOpenChange,
  onArchived,
}: {
  org: string;
  project: string;
  source: ExternalSourceView;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onArchived: () => void;
}) {
  const archive = useApiMutation({
    mutationFn: async () =>
      data(
        await api.POST("/platform/v1/orgs/{org}/projects/{project}/sources/{source}/archive", {
          params: { path: { org, project, source: source.id } },
        }),
      ),
    invalidates: [sourcesKey(org, project)],
    onSuccess: () => {
      toast(t("project.sources.archived", "Source archived"));
      onArchived();
    },
  });
  return (
    <AlertDialog
      open={open}
      onOpenChange={onOpenChange}
      tone="danger"
      title={t("project.sources.archiveTitle", "Archive {name}?", { name: source.name })}
      description={t("project.sources.archiveConfirmHint", "Its key stops working and Kaeso refuses what it sends; archiving is permanent.")}
      confirmLabel={t("project.sources.archive", "Archive source")}
      loading={archive.isPending}
      onConfirm={() => archive.mutateAsync()}
    />
  );
}
