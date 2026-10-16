import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AppFrame, Badge, Button, Empty, Icon, icons, Input, List, Pill, StepDialog, Text } from "@logicplanes/kaeso-ui";
import { ScopeTabs } from "~/kaeso/scope-tabs";
import { toast } from "~/shell/toast";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import { allKeysQuery } from "~/api/queries/keys";
import { serviceAccountsQuery } from "~/api/queries/members";
import { useApiMutation } from "~/auth/mutation";
import { useCatalogue } from "~/kaeso/catalogue";
import { grantLabel, type Grant } from "~/kaeso/grant";
import { GrantEditor } from "~/kaeso/rights-editor";
import { rolesOfScope, useRoles } from "~/kaeso/roles";
import { usePageCreateAction } from "~/shell/create-action";
import { ProjectHeader } from "~/shell/project-header";
import { usePartyName } from "../keys/party";
import { ServiceAccountSheet } from "./service-account-sheet";
import { useProjectRights } from "~/auth/rights";
import { useProjectGiver } from "~/auth/givers";
import { useNeedsRight } from "~/auth/needs-right";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { patchSearch, type SearchPatch } from "~/search";

export type ServiceAccountsSearch = { account?: string; archived?: boolean; create?: boolean };

export function ServiceAccountsPage({ org, project, search }: { org: string; project: string; search: ServiceAccountsSearch }) {
  const needs = useNeedsRight(org, project);
  const navigate = useNavigate();
  const rights = useProjectRights(org, project);
  const accounts = useQuery(serviceAccountsQuery(org, project, !!search.archived));
  const keys = useQuery({ ...allKeysQuery(org, project), enabled: rights.has("keys.all.view") });
  const partyName = usePartyName(org, project);
  const [creating, setCreating] = useState(!!search.create);
  const canCreate = rights.has("service_accounts.create");
  const reason = canCreate ? undefined : needs("service_accounts.create");
  usePageCreateAction({ label: t("project.serviceAccounts.create", "Create service account"), run: () => setCreating(true), disabledReason: reason });

  const setSearch = (patch: SearchPatch<ServiceAccountsSearch>) => {
    void navigate({
      to: "/orgs/$org/projects/$project/service-accounts",
      params: { org, project },
      search: patchSearch(search, patch, ["create"]),
      replace: true,
    });
  };

  const list = useMemo(() => [...(accounts.data ?? [])].sort((a, b) => a.name.localeCompare(b.name)), [accounts.data]);
  const keyCount = (id: string) => (keys.data ?? []).filter((k) => k.principal.kind === "service_account" && k.principal.id === id).length;
  const items = list.map((a) => {
    const n = keyCount(a.id);
    return {
      id: a.id,
      textValue: a.name,
      title: a.name,
      glyph: <Icon icon={icons.serviceAccount} size="sm" />,
      subtitle: a.created_by ? t("project.serviceAccounts.createdBy", "Created by {name}", { name: partyName({ kind: "user", id: a.created_by }) }) : undefined,
      meta: (
        <span className="flex items-center gap-8">
          {a.archived_at && <Badge>{common.archived()}</Badge>}
          {keys.data && (
            <Text variant="small" tone="secondary" tabular>
              {t("project.serviceAccounts.keyCount", "{count, plural, one {# key} other {# keys}}", { count: n })}
            </Text>
          )}
          <Pill size="sm">{grantLabel(a.grant)}</Pill>
        </span>
      ),
      date: formatDate(a.created_at),
    };
  });

  return (
    <>
      <ProjectHeader
        org={org}
        project={project}
        page={t("project.serviceAccounts.title", "Service accounts")}
        path="service-accounts"
        tabs={
          <ScopeTabs
            label={t("project.serviceAccounts.title", "Service accounts")}
            all={!!search.archived}
            onChange={(v) => setSearch({ archived: v || undefined })}
          />
        }
        actions={
          <Button
            size="sm"
            variant="primary"
            leadingIcon={icons.add}
            disabled={!canCreate}
            {...(reason ? { disabledReason: reason } : {})}
            onClick={() => setCreating(true)}
          >
            {t("project.serviceAccounts.create", "Create service account")}
          </Button>
        }
      />
      <AppFrame.Body padding="list">
        {accounts.isPending ? (
          <List aria-label={t("project.serviceAccounts.title", "Service accounts")} items={[]} loading />
        ) : list.length === 0 ? (
          <Empty
            size="lg"
            title={t("project.serviceAccounts.none", "No service accounts yet")}
            description={t("project.serviceAccounts.noneHint", "Give a deployment or a pipeline keys that outlast the people who made them.")}
            actions={
              canCreate ? (
                <Button variant="primary" onClick={() => setCreating(true)}>
                  {t("project.serviceAccounts.create", "Create service account")}
                </Button>
              ) : undefined
            }
          />
        ) : (
          <List aria-label={t("project.serviceAccounts.title", "Service accounts")} items={items} onOpen={(item) => setSearch({ account: item.id })} />
        )}
      </AppFrame.Body>
      <ServiceAccountSheet org={org} project={project} accountId={search.account} onClose={() => setSearch({ account: undefined })} />
      {creating && (
        <CreateServiceAccountDialog
          org={org}
          project={project}
          onClose={() => {
            setCreating(false);
            if (search.create) setSearch({});
          }}
          onCreated={(id) => setSearch({ account: id })}
        />
      )}
    </>
  );
}

function CreateServiceAccountDialog({
  org,
  project,
  onClose,
  onCreated,
}: {
  org: string;
  project: string;
  onClose: () => void;
  onCreated: (id: string) => void;
}) {
  const giver = useProjectGiver(org, project);
  const cat = useCatalogue(org);
  const roles = useRoles(org);
  const [name, setName] = useState("");
  const [grant, setGrant] = useState<Grant>({ role: "developer", rights_added: [], rights_removed: [] });
  const create = useApiMutation({
    mutationFn: async () =>
      data(
        await api.POST("/platform/v1/orgs/{org}/projects/{project}/service-accounts", {
          params: { path: { org, project } },
          body: { name: name.trim(), ...grant },
        }),
      ),
    invalidates: [["orgs", org, "projects", project, "service-accounts"]],
    onSuccess: (a) => {
      toast(t("project.serviceAccounts.created", "Service account {name} created", { name: a.name }));
      onClose();
      onCreated(a.id);
    },
  });
  return (
    <StepDialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t("project.serviceAccounts.create", "Create service account")}
      submitLabel={t("project.serviceAccounts.create", "Create service account")}
      submitting={create.isPending}
      onSubmit={() => name.trim() && create.mutate()}
      size="md"
      steps={[
        {
          id: "name",
          title: t("project.serviceAccounts.whatUses", "What uses it?"),
          description: t("project.serviceAccounts.whatUsesHint", "A service or a job, like checkout-production; you create its keys next."),
          blockedReason: name.trim() ? undefined : t("project.serviceAccounts.giveName", "Give it a name"),
          content: (
            <Input
              label={common.name()}
              placeholder={t("project.serviceAccounts.nameExample", "checkout-production")}
              value={name}
              onChange={(e) => setName(e.target.value)}
              autoFocus
              maxLength={100}
            />
          ),
        },
        {
          id: "rights",
          title: t("project.serviceAccounts.whatCan", "What can its keys do?"),
          description: t("project.serviceAccounts.whatCanHint", "Its role in this project; its keys act with it."),
          content: <GrantEditor cat={cat} scope="project" roles={rolesOfScope(roles, "project")} value={grant} onChange={setGrant} giver={giver} />,
        },
      ]}
    />
  );
}
