import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AppFrame, Avatar, Button, Combobox, Empty, KeyDisplay, List, Text, icons, type ListGroup } from "@logicplanes/kaeso-ui";
import { formatRelative } from "@logicplanes/kaeso-ui/format";
import { allKeysQuery } from "~/api/queries/keys";
import { projectMembersQuery, serviceAccountsQuery } from "~/api/queries/members";
import { keyStateLabel } from "~/format/labels";
import { CreateKeyDialog } from "~/kaeso/create-key-flow";
import { expiryText } from "./expiry";
import { usePageCreateAction } from "~/shell/create-action";
import { ScopeTabs } from "~/kaeso/scope-tabs";
import { ProjectHeader } from "~/shell/project-header";
import { KeySheet } from "./key-sheet";
import { asKeyState, serviceAccountName, usePartyName } from "./party";
import { useProjectRights } from "~/auth/rights";
import { useNeedsRight } from "~/auth/needs-right";
import { useSession } from "~/auth/session";
import { t } from "~/i18n";

export type KeysSearch = { key?: string; owner?: string; archived?: boolean; create?: boolean };

const STATES = ["active", "disabled", "expired", "archived"] as const;

export function KeysPage({ org, project, search }: { org: string; project: string; search: KeysSearch }) {
  const needs = useNeedsRight(org, project);
  const navigate = useNavigate();
  const session = useSession();
  const rights = useProjectRights(org, project);
  const [creating, setCreating] = useState(!!search.create);
  const owner = search.owner;
  const filter = {
    ...(owner?.startsWith("user:") ? { user: owner.slice(5) } : {}),
    ...(owner?.startsWith("sa:") ? { service_account: owner.slice(3) } : {}),
    ...(search.archived ? { include_archived: true } : {}),
  };
  const keys = useQuery(allKeysQuery(org, project, filter));
  const members = useQuery({ ...projectMembersQuery(org, project), enabled: rights.has("project.members.view") });
  const accounts = useQuery({ ...serviceAccountsQuery(org, project), enabled: rights.has("service_accounts.view") });
  const partyName = usePartyName(org, project);
  const canCreate = rights.has("keys.own.create");

  usePageCreateAction({
    label: t("keys.create", "Create key"),
    run: () => setCreating(true),
    disabledReason: canCreate ? undefined : needs("keys.own.create"),
  });

  const setSearch = (patch: Partial<Record<keyof KeysSearch, string | boolean | undefined>>) => {
    const next = Object.fromEntries(Object.entries({ ...search, ...patch, create: undefined }).filter(([, v]) => v !== undefined && v !== false)) as KeysSearch;
    void navigate({ to: "/orgs/$org/projects/$project/keys", params: { org, project }, search: next, replace: true });
  };

  const list = useMemo(() => keys.data ?? [], [keys.data]);
  const groups: ListGroup[] = useMemo(
    () =>
      STATES.map((state) => ({
        id: state,
        title: keyStateLabel(state),
        count: list.filter((k) => k.state === state).length,
        items: list
          .filter((k) => k.state === state)
          .map((k) => {
            const expiry = expiryText(k);
            const who = partyName(k.principal);
            return {
              id: k.id,
              textValue: k.name,
              title: k.name,
              subtitle: (
                <span className="hidden min-w-0 sm:inline-flex">
                  <KeyDisplay redacted={k.redacted} state={asKeyState(k.state)} copy={false} {...(k.access !== "inference" ? { access: k.access } : {})} />
                </span>
              ),
              meta: (
                <Text variant="small" tone={expiry.soon ? "warning" : "secondary"} className="hidden min-w-120 text-right sm:inline">
                  {expiry.text}
                </Text>
              ),
              date: k.last_used_at ? formatRelative(k.last_used_at) : t("keys.neverUsed", "Never used"),
              avatar: <Avatar name={k.principal.kind === "user" && k.principal.id === session.user.id ? session.user.name : who} size={20} />,
            };
          }),
      })).filter((g) => g.items.length > 0),
    [list, partyName, session.user.id, session.user.name],
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
        tabs={<ScopeTabs label={t("keys.keys", "Keys")} all={!!search.archived} onChange={(v) => setSearch({ archived: v || undefined })} />}
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
        ) : list.length === 0 ? (
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
          <List aria-label={t("keys.keys", "Keys")} groups={groups} onOpen={(item) => setSearch({ key: item.id })} />
        )}
      </AppFrame.Body>
      <KeySheet org={org} project={project} keyId={search.key} onClose={() => setSearch({ key: undefined })} />
      <CreateKeyDialog org={org} project={project} open={creating} onClose={() => setCreating(false)} />
    </>
  );
}
