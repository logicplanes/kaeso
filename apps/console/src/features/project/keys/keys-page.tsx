import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AppFrame, Avatar, Button, Combobox, Empty, FilterRow, Input, KeyDisplay, List, Select, Text, icons, type ListGroup } from "@logicplanes/kaeso-ui";
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
import { findKeys, isKeySort, keySortOptions, type KeySort } from "~/kaeso/key-list";

export type KeysSearch = { key?: string; owner?: string; scope?: "archived" | "all"; sort?: KeySort; create?: boolean };

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

  const [query, setQuery] = useState("");
  const list = useMemo(() => findKeys(keys.data ?? [], query, search.sort), [keys.data, query, search.sort]);
  const narrowed = !!owner || !!query.trim();
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
                  <KeyDisplay
                    redacted={k.redacted}
                    state={asKeyState(k.state)}
                    stateBadge={false}
                    copy={false}
                    {...(k.access !== "inference" ? { access: k.access } : {})}
                  />
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
        <FilterRow>
          {ownerOptions.length > 0 && (
            <div className="w-200">
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
          <Select
            aria-label={t("keys.sort", "Sort")}
            size="sm"
            icon={icons.sort}
            value={search.sort ?? "name"}
            onValueChange={(v) => setSearch({ sort: isKeySort(v) ? v : undefined })}
            options={keySortOptions()}
          />
          <div className="min-w-0 flex-1 basis-160 sm:w-240 sm:flex-none">
            <Input
              type="search"
              size="sm"
              aria-label={t("keys.search", "Search keys")}
              placeholder={t("keys.nameOrKey", "Name or key")}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onClear={() => setQuery("")}
            />
          </div>
        </FilterRow>
        {keys.isPending ? (
          <List aria-label={t("keys.keys", "Keys")} items={[]} loading />
        ) : keys.isError ? (
          <LoadFailed title={t("keys.listFailed", "The keys couldn't load")} size="md" />
        ) : narrowed && groups.length === 0 ? (
          <Empty
            size="lg"
            title={t("keys.noneForFilter", "No keys for this filter")}
            description={t("keys.clearFiltersHint", "Clear the search and the filter to see every key.")}
            actions={
              <Button
                onClick={() => {
                  setQuery("");
                  setSearch({ owner: undefined });
                }}
              >
                {t("keys.clearFilter", "Clear filter")}
              </Button>
            }
          />
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
            title={t("keys.none", "No keys yet")}
            description={t("keys.noneHint", "Create a key, then send it with every request to the gateway.")}
            actions={
              canCreate ? (
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
