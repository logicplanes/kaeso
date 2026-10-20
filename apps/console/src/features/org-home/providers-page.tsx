import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  AlertDialog,
  AppFrame,
  Badge,
  Button,
  Checkbox,
  CheckboxGroup,
  Code,
  DataTable,
  Dialog,
  Empty,
  FilterRow,
  IconButton,
  Input,
  Menu,
  ProviderBadge,
  SearchInput,
  Text,
  Tooltip,
  col,
  icons,
} from "@logicplanes/kaeso-ui";
import { formatDate, formatRelative } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import type { CredentialView } from "~/api/generated/types";
import { orgCredentialsQuery } from "~/api/queries/credentials";
import { orgProjectsQuery } from "~/api/queries/projects";
import { useApiMutation } from "~/auth/mutation";
import { useOrgRights } from "~/auth/session";
import { useNeedsRight } from "~/auth/needs-right";
import { useOrgPartyName } from "~/features/org/party";
import { providerLabel } from "~/format/labels";
import { docsPage } from "~/format/docs";
import { ProviderKeyDialog } from "~/kaeso/provider-key-dialog";
import { usePageCreateAction } from "~/shell/create-action";
import { OrgHeader } from "~/shell/org-header";
import { toast } from "~/shell/toast";
import { t } from "~/i18n";

type Row = { id: string; provider: string; key: CredentialView; first: boolean };

export const SEARCH_ABOVE = 8;

function matches(key: CredentialView, query: string): boolean {
  const words = [providerLabel(key.provider), key.provider, key.name, ...key.projects.map((p) => p.name)].map((w) => w.toLocaleLowerCase());
  return query
    .toLocaleLowerCase()
    .split(/\s+/)
    .filter(Boolean)
    .every((part) => words.some((w) => w.includes(part)));
}

function rowsOf(keys: CredentialView[]): Row[] {
  const providers = [...new Set(keys.map((k) => k.provider))].sort((a, b) => providerLabel(a).localeCompare(providerLabel(b)));
  return providers.flatMap((provider) => keys.filter((k) => k.provider === provider).map((key, i) => ({ id: key.id, provider, key, first: i === 0 })));
}

const joined = (names: string[]) => names.join(t("orgHome.listSeparator", ", "));

function namesLine(names: string[]): string {
  if (names.length <= 2) return joined(names);
  return t("orgHome.namesAndMore", "{names} and {count} more", { names: joined(names.slice(0, 2)), count: names.length - 2 });
}

function assignment(k: CredentialView): string {
  const names = k.projects.map((p) => p.name);
  if (names.length > 0)
    return k.is_default ? t("orgHome.projectsAndUnassigned", "{projects}, and every unassigned project", { projects: namesLine(names) }) : namesLine(names);
  return k.is_default ? t("orgHome.everyUnassignedProject", "Every unassigned project") : t("orgHome.noProject", "No project");
}

const lastUsed = (k: CredentialView) => (k.last_used_at ? formatRelative(k.last_used_at) : t("common.never", "Never"));

export const keyTitle = (k: CredentialView) => t("orgHome.keyTitle", "{provider} · {name}", { provider: providerLabel(k.provider), name: k.name });

export function OrgProvidersPage({ org }: { org: string }) {
  const rights = useOrgRights(org);
  const needs = useNeedsRight(org);
  const canView = rights.has("provider_credentials.view");
  const canAdd = rights.has("provider_credentials.create");
  const canChange = rights.has("provider_credentials.replace");
  const canDelete = rights.has("provider_credentials.delete");
  const credentials = useQuery({ ...orgCredentialsQuery(org), enabled: canView });
  const partyName = useOrgPartyName(org);
  const [adding, setAdding] = useState(false);
  const [replacing, setReplacing] = useState<CredentialView | null>(null);
  const [renaming, setRenaming] = useState<CredentialView | null>(null);
  const [assigning, setAssigning] = useState<CredentialView | null>(null);
  const [deleting, setDeleting] = useState<CredentialView | null>(null);
  const [query, setQuery] = useState("");
  const [hidden, setHidden] = useState<string[]>([]);
  const keys = useMemo(() => credentials.data ?? [], [credentials.data]);
  const searchable = keys.length > SEARCH_ABOVE;
  const shown = searchable && query.trim() ? keys.filter((k) => matches(k, query)) : keys;
  const rows = credentials.data ? rowsOf(shown) : [];
  const addReason = canAdd ? undefined : needs("provider_credentials.create");
  const addLabel = t("orgHome.addProviderKey", "Add provider key");
  usePageCreateAction(canView ? { label: addLabel, run: () => setAdding(true), disabledReason: addReason } : null);

  const makeDefault = useApiMutation({
    mutationFn: async (k: CredentialView) =>
      data(await api.POST("/platform/v1/orgs/{org}/credentials/{credential}/default", { params: { path: { org, credential: k.id } } })),
    invalidates: [["orgs", org, "credentials"]],
    onSuccess: (k) => toast(t("orgHome.keyMadeDefault", "{name} is the {provider} default", { name: k.name, provider: providerLabel(k.provider) })),
  });
  const remove = useApiMutation({
    mutationFn: async (k: CredentialView) => {
      await api.DELETE("/platform/v1/orgs/{org}/credentials/{credential}", { params: { path: { org, credential: k.id } } });
    },
    invalidates: [["orgs", org, "credentials"]],
    onSuccess: (_r, k) => {
      setDeleting(null);
      toast(t("orgHome.keyDeleted", "{title} deleted", { title: keyTitle(k) }));
    },
    onError: () => setDeleting(null),
  });

  const menu = (k: CredentialView) => {
    if (!canChange && !canDelete) return null;
    return (
      <Menu>
        <Menu.Trigger>
          <IconButton icon={icons.more} aria-label={t("orgHome.actionsForKey", "Actions for {title}", { title: keyTitle(k) })} size="xs" />
        </Menu.Trigger>
        <Menu.Content align="end">
          {canChange && !k.is_default && (
            <Menu.Item icon={icons.home} onClick={() => makeDefault.mutate(k)}>
              {t("orgHome.makeDefault", "Make default")}
            </Menu.Item>
          )}
          {canChange && (
            <Menu.Item icon={icons.route} onClick={() => setAssigning(k)}>
              {t("orgHome.assignToProjects", "Assign to projects")}
            </Menu.Item>
          )}
          {canChange && (
            <Menu.Item icon={icons.providerKey} onClick={() => setReplacing(k)}>
              {t("orgHome.replace", "Replace")}
            </Menu.Item>
          )}
          {canChange && (
            <Menu.Item icon={icons.edit} onClick={() => setRenaming(k)}>
              {t("common.rename", "Rename")}
            </Menu.Item>
          )}
          {canDelete && (
            <Menu.Item icon={icons.delete} variant="danger" onClick={() => setDeleting(k)}>
              {t("common.delete", "Delete")}
            </Menu.Item>
          )}
        </Menu.Content>
      </Menu>
    );
  };

  const columns = [
    col.custom(
      { id: "provider", header: t("common.provider", "Provider"), size: 208, minWidth: 128, priority: 1, grow: true },
      (r: Row) => r,
      (r: Row) => (
        <span className="flex min-w-0 items-center gap-8 sm:gap-12">
          {r.first ? <ProviderBadge provider={r.provider} size="lg" /> : <span aria-hidden className="w-28 shrink-0" />}
          <Text weight={r.first ? "medium" : undefined} tone={r.first ? "default" : "tertiary"} truncate>
            {providerLabel(r.provider)}
          </Text>
        </span>
      ),
    ),
    col.custom(
      { id: "key", header: t("orgHome.key", "Key"), size: 240, priority: 1, grow: true },
      (r: Row) => r,
      (r: Row) => {
        const under = [hidden.includes("projects") ? assignment(r.key) : null, hidden.includes("used") ? lastUsed(r.key) : null].filter(
          (part): part is string => !!part,
        );
        return (
          <span className="flex min-w-0 flex-col">
            <span className="flex min-w-0 items-center gap-8">
              <Text truncate>{r.key.name}</Text>
              <Tooltip content={t("orgHome.masterKeyId", "Encrypted with the master key {id}", { id: r.key.key_id })}>
                <Code className="hidden sm:inline">{`…${r.key.last4}`}</Code>
              </Tooltip>
              {r.key.is_default && <Badge>{t("orgHome.default", "Default")}</Badge>}
            </span>
            {under.length > 0 && (
              <Text variant="small" tone="secondary" truncate>
                {under.join(t("orgHome.partSeparator", " · "))}
              </Text>
            )}
          </span>
        );
      },
    ),
    col.custom(
      { id: "projects", header: t("common.projects", "Projects"), size: 220, priority: 2 },
      (r: Row) => r.key,
      (k: CredentialView) => (
        <Tooltip content={k.projects.length > 0 ? joined(k.projects.map((p) => p.name)) : undefined}>
          <Text variant="small" tone={k.projects.length > 0 || k.is_default ? "secondary" : "tertiary"} truncate>
            {assignment(k)}
          </Text>
        </Tooltip>
      ),
    ),
    col.custom(
      { id: "added", header: t("orgHome.added", "Added"), size: 216, priority: 3 },
      (r: Row) => r.key,
      (k: CredentialView) => (
        <Text variant="small" tone="secondary" truncate>
          {t("orgHome.addedBy", "{name}, {date}", { name: partyName(k.created_by), date: formatDate(k.created_at) })}
        </Text>
      ),
    ),
    col.custom(
      { id: "used", header: t("common.lastUsed", "Last used"), size: 136, priority: 2 },
      (r: Row) => r.key,
      (k: CredentialView) => (
        <Text variant="small" tone={k.last_used_at ? "secondary" : "tertiary"}>
          {lastUsed(k)}
        </Text>
      ),
    ),
    col.custom(
      { id: "actions", header: "", size: 56, priority: 1 },
      (r: Row) => r.key,
      (k: CredentialView) => menu(k),
      { align: "end" },
    ),
  ].map((c) => ({ ...c, enableSorting: false }));

  const deletingProvider = deleting ? providerLabel(deleting.provider) : "";
  const successor = deleting?.is_default
    ? keys.filter((k) => k.provider === deleting.provider && k.id !== deleting.id).sort((a, b) => b.created_at.localeCompare(a.created_at))[0]
    : undefined;

  return (
    <>
      <OrgHeader
        org={org}
        page={t("orgHome.providers", "Providers")}
        actions={
          canView &&
          !(credentials.isSuccess && keys.length === 0) && (
            <Button
              size="sm"
              variant="primary"
              leadingIcon={icons.add}
              disabled={!canAdd}
              {...(addReason ? { disabledReason: addReason } : {})}
              onClick={() => setAdding(true)}
            >
              {addLabel}
            </Button>
          )
        }
      />
      {canView && searchable && (
        <FilterRow>
          <div className="w-240">
            <SearchInput
              aria-label={t("orgHome.searchProviderKeys", "Search provider keys")}
              placeholder={t("orgHome.providerKeyOrProject", "Provider, key or project")}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onClear={() => setQuery("")}
            />
          </div>
        </FilterRow>
      )}
      <AppFrame.Body padding="list">
        {!canView ? (
          <Empty title={t("orgHome.providersHidden", "You can't see the providers here")} description={needs("provider_credentials.view")} />
        ) : (
          <DataTable
            aria-label={t("orgHome.providerKeys", "Provider keys")}
            data={rows}
            columns={columns}
            getRowId={(r) => r.id}
            onHiddenColumnsChange={setHidden}
            density="comfortable"
            loading={credentials.isPending}
            error={credentials.isError && rows.length === 0 ? t("orgHome.providerKeysFailed", "The provider keys couldn't load") : undefined}
            onRetry={() => void credentials.refetch()}
            empty={
              keys.length > 0 ? (
                <Empty
                  size="md"
                  title={t("orgHome.noProviderKeysMatch", "No provider keys match")}
                  description={t("orgHome.searchAnotherProvider", "Search by another provider, key or project name.")}
                />
              ) : (
                <Empty
                  title={t("orgHome.noProviderKeys", "No provider keys yet")}
                  description={t("orgHome.noProviderKeysHint", "A provider key lets this organization's projects call that provider's models.")}
                  actions={
                    <>
                      {canAdd && (
                        <Button variant="primary" leadingIcon={icons.add} onClick={() => setAdding(true)}>
                          {addLabel}
                        </Button>
                      )}
                      <Button variant="ghost" href={docsPage("/console/provider-keys")}>
                        {t("orgHome.aboutProviderKeys", "About provider keys")}
                      </Button>
                    </>
                  }
                />
              )
            }
          />
        )}
      </AppFrame.Body>
      {adding && <ProviderKeyDialog org={org} keys={keys} onClose={() => setAdding(false)} />}
      {replacing && <ProviderKeyDialog org={org} keys={keys} replace={replacing} onClose={() => setReplacing(null)} />}
      {renaming && <RenameKeyDialog org={org} credential={renaming} onClose={() => setRenaming(null)} />}
      {assigning && <AssignKeyDialog org={org} credential={assigning} keys={keys} onClose={() => setAssigning(null)} />}
      <AlertDialog
        open={!!deleting}
        onOpenChange={(o) => !o && setDeleting(null)}
        tone="danger"
        title={deleting ? t("orgHome.deleteKeyQuestion", "Delete {title}?", { title: keyTitle(deleting) }) : t("orgHome.deleteThisKey", "Delete this key?")}
        description={
          !deleting?.is_default
            ? deleting?.projects.length
              ? t("orgHome.deleteKeyProjectsSwitch", "Its projects switch to the {provider} default.", { provider: deletingProvider })
              : t("orgHome.deleteKeyUnused", "No project uses it; requests are not affected.")
            : successor
              ? t("orgHome.deleteKeySuccessor", "{name} becomes the {provider} default.", { name: successor.name, provider: deletingProvider })
              : t("orgHome.deleteKeyLast", "Requests to {provider} fail until the organization adds another key.", { provider: deletingProvider })
        }
        confirmLabel={t("orgHome.deleteKey", "Delete key")}
        loading={remove.isPending}
        onConfirm={() => (deleting ? remove.mutateAsync(deleting) : undefined)}
      />
    </>
  );
}

function RenameKeyDialog({ org, credential, onClose }: { org: string; credential: CredentialView; onClose: () => void }) {
  const [name, setName] = useState(credential.name);
  const trimmed = name.trim();
  const rename = useApiMutation({
    mutationFn: async () =>
      data(
        await api.PATCH("/platform/v1/orgs/{org}/credentials/{credential}", {
          params: { path: { org, credential: credential.id } },
          body: { name: trimmed },
        }),
      ),
    invalidates: [["orgs", org, "credentials"]],
    onSuccess: (k) => {
      toast(t("orgHome.keyRenamed", "Renamed to {name}", { name: k.name }));
      onClose();
    },
  });
  const blocked = !trimmed
    ? t("orgHome.keyNameNeeded", "Give the key a name")
    : trimmed === credential.name
      ? t("orgHome.changeNameFirst", "Change the name first")
      : undefined;
  return (
    <Dialog
      busy={rename.isPending}
      open
      onOpenChange={(o) => !o && onClose()}
      size="sm"
      title={t("orgHome.renameKey", "Rename {title}", { title: keyTitle(credential) })}
      onSubmit={() => !blocked && rename.mutate()}
      footer={
        <>
          <Dialog.Close>
            <Button>{t("common.cancel", "Cancel")}</Button>
          </Dialog.Close>
          <Button
            variant="primary"
            disabled={!!blocked}
            {...(blocked ? { disabledReason: blocked } : {})}
            loading={rename.isPending}
            onClick={() => rename.mutate()}
          >
            {t("common.rename", "Rename")}
          </Button>
        </>
      }
    >
      <Input label={t("common.name", "Name")} value={name} onChange={(e) => setName(e.target.value)} maxLength={100} autoFocus />
    </Dialog>
  );
}

function AssignKeyDialog({ org, credential, keys, onClose }: { org: string; credential: CredentialView; keys: CredentialView[]; onClose: () => void }) {
  const projects = useQuery(orgProjectsQuery(org));
  const [chosen, setChosen] = useState<string[]>(credential.projects.map((p) => p.id));
  const label = providerLabel(credential.provider);
  const otherKey = new Map<string, string>();
  for (const k of keys) {
    if (k.provider !== credential.provider || k.id === credential.id) continue;
    for (const p of k.projects) otherKey.set(p.id, k.name);
  }
  const options = [
    ...(projects.data ?? []).map((p) => ({ id: p.id, name: p.name })),
    ...credential.projects.filter((p) => !(projects.data ?? []).some((q) => q.id === p.id)),
  ];
  const save = useApiMutation({
    mutationFn: async () =>
      data(
        await api.PUT("/platform/v1/orgs/{org}/credentials/{credential}/projects", {
          params: { path: { org, credential: credential.id } },
          body: { projects: chosen },
        }),
      ),
    invalidates: [["orgs", org, "credentials"]],
    onSuccess: (k) => {
      toast(
        k.projects.length === 0
          ? t("orgHome.keyUnassigned", "No project is assigned to {title}", { title: keyTitle(k) })
          : t("orgHome.keyAssigned", "{title} assigned to {count, plural, one {# project} other {# projects}}", {
              title: keyTitle(k),
              count: k.projects.length,
            }),
      );
      onClose();
    },
  });
  return (
    <Dialog
      busy={save.isPending}
      open
      onOpenChange={(o) => !o && onClose()}
      size="sm"
      title={t("orgHome.assignKey", "Assign {title}", { title: keyTitle(credential) })}
      description={
        credential.is_default
          ? t("orgHome.assignDefaultKeyHint", "These projects keep it for {provider} even when another key becomes the default.", { provider: label })
          : t("orgHome.assignKeyHint", "These projects use it for {provider} instead of the default.", { provider: label })
      }
      onSubmit={() => save.mutate()}
      footer={
        <>
          <Dialog.Close>
            <Button>{t("common.cancel", "Cancel")}</Button>
          </Dialog.Close>
          <Button variant="primary" loading={save.isPending} onClick={() => save.mutate()}>
            {t("common.save", "Save")}
          </Button>
        </>
      }
    >
      {projects.isPending ? (
        <Text tone="tertiary">{t("orgHome.loadingProjects", "Loading projects…")}</Text>
      ) : (
        <CheckboxGroup aria-label={t("common.projects", "Projects")} value={chosen} onValueChange={setChosen}>
          {options.map((p) => {
            const other = otherKey.get(p.id);
            return (
              <Checkbox
                key={p.id}
                value={p.id}
                label={p.name}
                {...(other ? { description: t("orgHome.usesKeyNow", "Uses {name} now", { name: other }) } : {})}
              />
            );
          })}
        </CheckboxGroup>
      )}
    </Dialog>
  );
}
