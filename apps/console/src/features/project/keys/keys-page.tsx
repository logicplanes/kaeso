import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AppFrame, Avatar, Button, Combobox, Empty, KeyDisplay, List, Text, icons, type ListGroup } from "@logicplanes/kaeso-ui";
import { allKeysQuery, keyFilter } from "~/api/queries/keys";
import { projectMembersQuery, serviceAccountsQuery } from "~/api/queries/members";
import { keyStateLabel } from "~/format/labels";
import { CreateKeyDialog } from "~/kaeso/create-key-flow";
import { expiryText } from "./expiry";
import { usePageCreateAction } from "~/shell/create-action";
import { ScopeTabs } from "~/kaeso/scope-tabs";
import { LoadFailed } from "~/features/org/load-failed";
import { ProjectHeader } from "~/shell/project-header";
import { KeySheet } from "./key-sheet";
import { asKeyState, serviceAccountName, usePartyLabel } from "./party";
import { useProjectRights } from "~/auth/rights";
import { useNeedsRight } from "~/auth/needs-right";
import { t } from "~/i18n";
import { patchSearch, type SearchPatch } from "~/search";
import { LastUsed } from "~/kaeso/last-used";

export type KeysSearch = { key?: string; owner?: string; scope?: "archived" | "all"; create?: boolean };

const STATES = ["active", "disabled", "expired", "archived"] as const;

export function KeysPage({ org, project, search }: { org: string; project: string; search: KeysSearch }) {
  const needs = useNeedsRight(org, project);
  const navigate = useNavigate();
  const rights = useProjectRights(org, project);
  const [creating, setCreating] = useState(!!search.create);
  const owner = search.owner;
  const scope = search.scope ?? "active";
  const keys = useQuery(allKeysQuery(org, project, keyFilter({ owner, archived: scope !== "active" })));
  const members = useQuery({ ...projectMembersQuery(org, project), enabled: rights.has("project.members.view") });
  const accounts = useQuery({ ...serviceAccountsQuery(org, project), enabled: rights.has("service_accounts.view") });
  const partyLabel = usePartyLabel(org, project);
  const canCreate = rights.has("keys.own.create");

  usePageCreateAction({
    label: t("keys.create", "Create key"),
    run: () => setCreating(true),
    disabledReason: canCreate ? undefined : needs("keys.own.create"),
  });

  const setSearch = (patch: SearchPatch<KeysSearch>) => {
    void navigate({ to: "/orgs/$org/projects/$project/keys", params: { org, project }, search: patchSearch(search, patch, ["create"]), replace: true });
  };

  const list = useMemo(() => keys.data ?? [], [keys.data]);
  const groups: ListGroup[] = useMemo(
    () =>
      (scope === "all" ? STATES : ([scope] as const))
        .map((state) => ({
          id: state,
          title: keyStateLabel(state),
          count: list.filter((k) => k.state === state).length,
          items: list
            .filter((k) => k.state === state)
            .map((k) => {
              const expiry = expiryText(k);
              const who = partyLabel(k.principal);
              return {
                id: k.id,
                textValue: k.name,
                title: k.name,
                subtitle: (
                  <KeyDisplay redacted={k.redacted} state={asKeyState(k.state)} copy={false} {...(k.access !== "inference" ? { access: k.access } : {})} />
                ),
                meta: (
                  <Text variant="small" tone={expiry.soon ? "warning" : "secondary"} className="sm:min-w-120 sm:text-right">
                    {expiry.text}
                  </Text>
                ),
                date: <LastUsed at={k.last_used_at} />,
                avatar: <Avatar name={who.name} src={who.avatarUrl} size={20} />,
              };
            }),
        }))
        .filter((g) => g.items.length > 0),
    [scope, list, partyLabel],
  );

  const ownerOptions = [
    ...(members.data ?? []).map((m) => ({ value: `user:${m.user.id}`, label: m.user.name })),
    ...(accounts.data ?? []).map((a) => ({ value: `sa:${a.id}`, label: serviceAccountName(a.name) })),
  ];

  return (
    <>
      <ProjectHeader
        org={org}
        project={project}
        page={t("keys.title", "API keys")}
        path="keys"
        tabs={<ScopeTabs label={t("keys.keys", "Keys")} value={scope} onChange={(v) => setSearch({ scope: v === "active" ? undefined : v })} />}
        actions={
          <>
            {ownerOptions.length > 0 && (
              <div className="hidden w-200 md:block">
                <Combobox
                  aria-label={t("keys.whose", "Whose keys")}
                  size="sm"
                  placeholder={t("keys.everyones", "Everyone's keys")}
                  options={ownerOptions}
                  value={owner ?? null}
                  onValueChange={(v: string | null) => setSearch({ owner: v ?? undefined })}
                />
              </div>
            )}
            <Button
              size="sm"
              variant="primary"
              leadingIcon={icons.add}
              disabled={!canCreate}
              {...(canCreate ? {} : { disabledReason: needs("keys.own.create") })}
              onClick={() => setCreating(true)}
            >
              {t("keys.create", "Create key")}
            </Button>
          </>
        }
      />
      <AppFrame.Body padding="list">
        {keys.isPending ? (
          <List aria-label={t("keys.keys", "Keys")} items={[]} loading />
        ) : keys.isError ? (
          <LoadFailed title={t("keys.listFailed", "The keys couldn't load")} size="md" />
        ) : scope === "archived" && groups.length === 0 ? (
          <Empty size="lg" title={t("keys.noneArchived", "No archived keys")} />
        ) : list.length > 0 && groups.length === 0 ? (
          <Empty
            size="lg"
            title={t("keys.noneActive", "No active keys")}
            description={t("keys.noneActiveHint", "Disabled and expired keys are under All.")}
            actions={<Button onClick={() => setSearch({ scope: "all" })}>{t("keys.showAll", "Show all keys")}</Button>}
          />
        ) : groups.length === 0 ? (
          <Empty
            size="lg"
            title={owner ? t("keys.noneForFilter", "No keys for this filter") : t("keys.none", "No keys yet")}
            description={
              owner
                ? t("keys.clearFilterHint", "Clear the filter to see everyone's keys.")
                : t("keys.noneHint", "Create a key, then send it with every request to the gateway.")
            }
            actions={
              owner ? (
                <Button onClick={() => setSearch({ owner: undefined })}>{t("keys.clearFilter", "Clear filter")}</Button>
              ) : canCreate ? (
                <Button variant="primary" onClick={() => setCreating(true)}>
                  {t("keys.create", "Create key")}
                </Button>
              ) : undefined
            }
          />
        ) : (
          <List
            aria-label={t("keys.keys", "Keys")}
            {...(scope === "all" ? { groups } : { items: groups.flatMap((g) => g.items) })}
            onOpen={(item) => setSearch({ key: item.id })}
          />
        )}
      </AppFrame.Body>
      <KeySheet org={org} project={project} keyId={search.key} onClose={() => setSearch({ key: undefined })} />
      <CreateKeyDialog
        org={org}
        project={project}
        open={creating}
        onClose={() => {
          setCreating(false);
          if (search.create) setSearch({});
        }}
      />
    </>
  );
}
