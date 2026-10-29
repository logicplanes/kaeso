import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  AlertDialog,
  Banner,
  Button,
  DialogScope,
  Empty,
  Input,
  KeyDisplay,
  KeyValue,
  List,
  Section,
  Sheet,
  Skeleton,
  Text,
  icons,
} from "@logicplanes/kaeso-ui";
import { toast } from "~/shell/toast";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import { isApiError } from "~/api/errors";
import type { ServiceAccountView } from "~/api/generated/types";
import { allKeysQuery } from "~/api/queries/keys";
import { useApiMutation } from "~/auth/mutation";
import { CreateKeyDialog } from "~/kaeso/create-key-flow";
import { asGrant, sameGrant, type Grant } from "~/kaeso/grant";
import { LimitsSection } from "~/kaeso/limits-section";
import { ModelAccessSection } from "~/kaeso/model-access-section";
import { GrantEditor } from "~/kaeso/rights-editor";
import { useCatalogue } from "~/kaeso/catalogue";
import { rolesOfScope, useRoles } from "~/kaeso/roles";
import { asKeyState, usePartyName } from "../keys/party";
import { useRememberRecent } from "~/shell/recent";
import { useProjectRights } from "~/auth/rights";
import { useProjectGiver } from "~/auth/givers";
import { useNeedsRight } from "~/auth/needs-right";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { LastUsed } from "~/kaeso/last-used";

const accountQuery = (org: string, project: string, account: string) => ({
  queryKey: ["orgs", org, "projects", project, "service-accounts", "one", account],
  queryFn: async (): Promise<ServiceAccountView> =>
    data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/service-accounts/{account}", { params: { path: { org, project, account } } })),
});

export function ServiceAccountSheet({
  org,
  project,
  accountId,
  onClose,
}: {
  org: string;
  project: string;
  accountId: string | undefined;
  onClose: () => void;
}) {
  const account = useQuery({ ...accountQuery(org, project, accountId ?? ""), enabled: !!accountId });
  return (
    <Sheet
      open={!!accountId}
      onOpenChange={(o) => !o && onClose()}
      modal={false}
      size="md"
      title={account.data?.name ?? t("project.serviceAccounts.one", "Service account")}
    >
      {accountId && <AccountBody key={accountId} org={org} project={project} accountId={accountId} onClose={onClose} />}
    </Sheet>
  );
}

const accountsKey = (org: string, project: string) => ["orgs", org, "projects", project, "service-accounts"];

function AccountBody({ org, project, accountId, onClose }: { org: string; project: string; accountId: string; onClose: () => void }) {
  const needs = useNeedsRight(org, project);
  const rights = useProjectRights(org, project);
  const giver = useProjectGiver(org, project);
  const cat = useCatalogue(org);
  const roles = useRoles(org);
  const account = useQuery(accountQuery(org, project, accountId));
  useRememberRecent(
    account.data
      ? { kind: "service-account", id: accountId, label: account.data.name, href: `/orgs/${org}/projects/${project}/service-accounts?account=${accountId}` }
      : null,
  );
  const keys = useQuery({ ...allKeysQuery(org, project, { service_account: accountId }), enabled: rights.has("keys.all.view") });
  const partyName = usePartyName(org, project);
  const [name, setName] = useState<string | null>(null);
  const [grant, setGrant] = useState<Grant | null>(null);
  const [creatingKey, setCreatingKey] = useState(false);
  const [archiving, setArchiving] = useState(false);

  const update = useApiMutation({
    mutationFn: async (body: { name?: string; grant?: Grant }) =>
      data(
        await api.PATCH("/platform/v1/orgs/{org}/projects/{project}/service-accounts/{account}", {
          params: { path: { org, project, account: accountId } },
          body,
        }),
      ),
    invalidates: [accountsKey(org, project)],
    onSuccess: (_a, body) => {
      if (body.name) setName(null);
      if (body.grant) setGrant(null);
      toast.success(body.name ? t("project.serviceAccounts.nameSaved", "Name saved") : t("project.serviceAccounts.rightsSaved", "Rights saved"));
    },
  });

  if (account.isPending) return <Skeleton className="w-full" />;
  if (account.isError) {
    return (
      <Empty
        title={
          isApiError(account.error) && account.error.type === "not_found"
            ? t("project.serviceAccounts.notFound", "This service account doesn't exist or was removed.")
            : t("project.serviceAccounts.loadFailed", "Couldn't load this service account.")
        }
        actions={<Button onClick={onClose}>{t("project.serviceAccounts.back", "Back to service accounts")}</Button>}
      />
    );
  }
  const a = account.data;
  const archived = !!a.archived_at;
  const canUpdate = rights.has("service_accounts.rights.update") && !archived;
  const current = asGrant(a.grant);
  const editing = grant ?? current;
  const changed = grant && !sameGrant(grant, current);
  const readOnly = archived
    ? t("project.serviceAccounts.archivedReadOnly", "Archived service accounts can't change")
    : canUpdate
      ? undefined
      : needs("service_accounts.rights.update");
  const canKeys = rights.has("service_accounts.keys.create");
  const activeKeys = (keys.data ?? []).filter((k) => k.state !== "archived");

  return (
    <div className="flex flex-col gap-24">
      {a.archived_at && (
        <Banner tone="neutral">
          {t("project.serviceAccounts.archivedOn", "Archived {date}. Its keys were archived with it.", { date: formatDate(a.archived_at) })}
        </Banner>
      )}
      {name !== null ? (
        <div className="flex items-end gap-8">
          <Input className="flex-1" label={common.name()} value={name} onChange={(e) => setName(e.target.value)} autoFocus maxLength={100} />
          <Button
            variant="primary"
            disabled={!name.trim() || name.trim() === a.name}
            loading={update.isPending && update.variables?.name !== undefined}
            onClick={() => update.mutate({ name: name.trim() })}
          >
            {common.save()}
          </Button>
          <Button onClick={() => setName(null)}>{common.cancel()}</Button>
        </div>
      ) : (
        <KeyValue
          items={[
            {
              label: common.name(),
              value: (
                <span className="flex flex-wrap items-center gap-x-8 gap-y-4">
                  <span className="min-w-0 wrap-break-word">{a.name}</span>
                  {!archived && (
                    <Button variant="ghost" size="xs" disabled={!!readOnly} {...(readOnly ? { disabledReason: readOnly } : {})} onClick={() => setName(a.name)}>
                      {common.rename()}
                    </Button>
                  )}
                </span>
              ),
            },
            {
              label: common.created(),
              value: a.created_by
                ? t("project.serviceAccounts.createdOnBy", "{date} by {name}", {
                    date: formatDate(a.created_at),
                    name: partyName({ kind: "user", id: a.created_by }),
                  })
                : formatDate(a.created_at),
            },
            { label: t("project.serviceAccounts.id", "Id"), value: a.id, mono: true, copy: a.id },
          ]}
        />
      )}
      <Section title={common.role()} description={t("project.serviceAccounts.roleHint", "What it and its keys can do in this project.")} spacing="none">
        <div className="flex flex-col gap-12">
          <GrantEditor cat={cat} scope="project" roles={rolesOfScope(roles, "project")} value={editing} onChange={setGrant} giver={giver} readOnly={readOnly} />
          {!archived && (
            <div className="flex gap-8">
              <Button
                size="sm"
                variant="primary"
                disabled={!changed}
                loading={update.isPending && update.variables?.grant !== undefined}
                onClick={() => update.mutate({ grant: editing })}
              >
                {t("project.serviceAccounts.saveRole", "Save role")}
              </Button>
              <Button size="sm" disabled={!changed} onClick={() => setGrant(null)}>
                {common.cancel()}
              </Button>
            </div>
          )}
        </div>
      </Section>
      {rights.has("keys.all.view") && (
        <Section
          title={t("project.serviceAccounts.keys", "Keys")}
          spacing="none"
          actions={
            archived ? undefined : (
              <Button
                size="sm"
                variant="ghost"
                leadingIcon={icons.add}
                disabled={!canKeys}
                {...(canKeys ? {} : { disabledReason: needs("service_accounts.keys.create") })}
                onClick={() => setCreatingKey(true)}
              >
                {t("project.serviceAccounts.createKey", "Create key")}
              </Button>
            )
          }
        >
          {activeKeys.length === 0 ? (
            <Text tone="secondary">
              {keys.isPending ? t("project.serviceAccounts.loadingKeys", "Loading keys…") : t("project.serviceAccounts.noKeys", "No keys yet.")}
            </Text>
          ) : (
            <List
              aria-label={t("project.serviceAccounts.keysOf", "Keys of {name}", { name: a.name })}
              density="compact"
              items={activeKeys.map((k) => ({
                id: k.id,
                textValue: k.name,
                title: k.name,
                subtitle: (
                  <KeyDisplay redacted={k.redacted} state={asKeyState(k.state)} copy={false} {...(k.access !== "inference" ? { access: k.access } : {})} />
                ),
                date: <LastUsed at={k.last_used_at} />,
              }))}
            />
          )}
        </Section>
      )}
      {rights.has("limits.view") && (
        <ModelAccessSection org={org} project={project} subject={{ kind: "service_account", id: a.id }} canEdit={rights.has("limits.manage") && !archived} />
      )}
      {rights.has("limits.view") && (
        <LimitsSection
          org={org}
          project={project}
          filter={{ service_account: a.id }}
          target={{ level: "member", service_account: a.id }}
          canEdit={rights.has("limits.manage") && !archived}
          empty={t("project.serviceAccounts.noLimits", "No limits on this service account; the project's limits apply.")}
        />
      )}
      {rights.has("service_accounts.archive") && !archived && (
        <Section title={common.archive()} spacing="none">
          <div className="flex flex-col items-start gap-12">
            <Text tone="secondary">{t("project.serviceAccounts.archiveHint", "Archiving is permanent; its history stays.")}</Text>
            <Button variant="danger-secondary" size="sm" onClick={() => setArchiving(true)}>
              {t("project.serviceAccounts.archive", "Archive service account")}
            </Button>
          </div>
        </Section>
      )}
      <DialogScope open={archiving}>
        <ArchiveAccountDialog org={org} project={project} account={a} onArchived={onClose} open={archiving} onOpenChange={setArchiving} />
      </DialogScope>
      <CreateKeyDialog org={org} project={project} open={creatingKey} onClose={() => setCreatingKey(false)} fixedServiceAccount={{ id: a.id, name: a.name }} />
    </div>
  );
}

function ArchiveAccountDialog({
  org,
  project,
  account,
  onArchived,
  open,
  onOpenChange,
}: {
  org: string;
  project: string;
  account: ServiceAccountView;
  onArchived: () => void;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const archive = useApiMutation({
    mutationFn: async () =>
      data(
        await api.POST("/platform/v1/orgs/{org}/projects/{project}/service-accounts/{account}/archive", {
          params: { path: { org, project, account: account.id } },
        }),
      ),
    invalidates: [accountsKey(org, project), ["orgs", org, "projects", project, "keys"]],
    onSuccess: () => {
      toast(t("project.serviceAccounts.archived", "Service account archived"));
      onArchived();
    },
  });
  return (
    <AlertDialog
      open={open}
      onOpenChange={onOpenChange}
      tone="danger"
      title={t("project.serviceAccounts.archiveTitle", "Archive {name}?", { name: account.name })}
      description={t("project.serviceAccounts.archiveConfirmHint", "Its keys are archived with it and stop authenticating; archiving is permanent.")}
      confirmLabel={t("project.serviceAccounts.archive", "Archive service account")}
      loading={archive.isPending}
      onConfirm={() => archive.mutateAsync()}
    />
  );
}
