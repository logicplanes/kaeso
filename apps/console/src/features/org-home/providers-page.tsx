import { useDeferredValue, useMemo, useState, type ReactElement } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
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
  PillTabs,
  ProviderBadge,
  SearchInput,
  Text,
  Tooltip,
  VisuallyHidden,
  col,
  icons,
} from "@logicplanes/kaeso-ui";
import { formatDate, formatRelative } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import type { CredentialView } from "~/api/generated/types";
import { orgCredentialsQuery } from "~/api/queries/credentials";
import { customProvidersQuery } from "~/api/queries/custom-providers";
import { orgProjectsQuery } from "~/api/queries/projects";
import { useApiMutation } from "~/auth/mutation";
import { useOrgRights } from "~/auth/session";
import { useNeedsRight } from "~/auth/needs-right";
import { useOrgPartyName } from "~/features/org/party";
import { docsPage } from "~/format/docs";
import { ProviderKeyDialog } from "~/kaeso/provider-key-dialog";
import { keyTitle, useProviders, type Providers } from "~/kaeso/providers";
import { usePageCreateAction } from "~/shell/create-action";
import { OrgHeader } from "~/shell/org-header";
import { CustomProvidersTab } from "./custom-providers";
import { toast } from "~/shell/toast";
import { t } from "~/i18n";

type Row = { id: string; provider: string; key: CredentialView; first: boolean };

export interface ProvidersSearch {
  tab?: "custom";
}

export const SEARCH_ABOVE = 8;

const searchWords = (key: CredentialView, catalog: Providers) =>
  [catalog.name(key.provider), key.provider, key.name, key.account ?? "", ...key.projects.map((p) => p.name)].map((w) => w.toLocaleLowerCase());

function rowsOf(keys: CredentialView[], catalog: Providers): Row[] {
  const byProvider = new Map<string, CredentialView[]>();
  for (const k of keys) byProvider.set(k.provider, [...(byProvider.get(k.provider) ?? []), k]);
  return [...byProvider.keys()]
    .sort(catalog.byName)
    .flatMap((provider) => (byProvider.get(provider) ?? []).map((key, i) => ({ id: key.id, provider, key, first: i === 0 })));
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

function withProjectNames(k: CredentialView, text: ReactElement) {
  return k.projects.length > 0 ? <Tooltip content={joined(k.projects.map((p) => p.name))}>{text}</Tooltip> : text;
}

const KEY_WIDTH = 240;
const ACCOUNT_KEY_WIDTH = 456;

const lastUsed = (k: CredentialView) => (k.last_used_at ? formatRelative(k.last_used_at) : t("common.never", "Never"));

export function OrgProvidersPage({ org, search = {} }: { org: string; search?: ProvidersSearch }) {
  const rights = useOrgRights(org);
  const needs = useNeedsRight(org);
  const navigate = useNavigate();
  const canSeeCustom = rights.has("providers.custom.view");
  const canManageCustom = rights.has("providers.custom.manage");
  const tab = search.tab === "custom" && canSeeCustom ? "custom" : "keys";
  const custom = useQuery({ ...customProvidersQuery(org), enabled: canSeeCustom });
  const [addingCustom, setAddingCustom] = useState(false);
  const canView = rights.has("provider_credentials.view");
  const canAdd = rights.has("provider_credentials.create");
  const canChange = rights.has("provider_credentials.replace");
  const canDelete = rights.has("provider_credentials.delete");
  const credentials = useQuery({ ...orgCredentialsQuery(org), enabled: canView });
  const providers = useProviders(org);
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
  const accounts = keys.some((k) => !!k.account);
  if (!searchable && query) setQuery("");
  const searched = useDeferredValue(searchable ? query : "");
  const words = useMemo(() => new Map(keys.map((k) => [k.id, searchWords(k, providers)])), [keys, providers]);
  const rows = useMemo(() => {
    const parts = searched.toLocaleLowerCase().split(/\s+/).filter(Boolean);
    const shown = parts.length ? keys.filter((k) => parts.every((part) => (words.get(k.id) ?? []).some((w) => w.includes(part)))) : keys;
    return credentials.data ? rowsOf(shown, providers) : [];
  }, [credentials.data, keys, words, searched, providers]);
  const addReason = canAdd ? undefined : needs("provider_credentials.create");
  const addLabel = t("orgHome.addProviderKey", "Add provider key");
  const customReason = canManageCustom ? undefined : needs("providers.custom.manage");
  const customLabel = t("orgHome.custom.add", "Add custom provider");
  usePageCreateAction(
    tab === "custom"
      ? { label: customLabel, run: () => setAddingCustom(true), disabledReason: customReason }
      : canView
        ? { label: addLabel, run: () => setAdding(true), disabledReason: addReason }
        : null,
  );
  const tabs = canSeeCustom ? (
    <PillTabs
      value={tab}
      onValueChange={(v) => void navigate({ to: "/orgs/$org/providers", params: { org }, search: v === "custom" ? { tab: "custom" } : {}, replace: true })}
    >
      <PillTabs.List aria-label={t("orgHome.providers", "Providers")}>
        <PillTabs.Tab value="keys" {...(credentials.data ? { count: credentials.data.length } : {})}>
          {t("orgHome.providerKeys", "Provider keys")}
        </PillTabs.Tab>
        <PillTabs.Tab value="custom" {...(custom.data ? { count: custom.data.length } : {})}>
          {t("orgHome.customProviders", "Custom providers")}
        </PillTabs.Tab>
      </PillTabs.List>
    </PillTabs>
  ) : undefined;

  const makeDefault = useApiMutation({
    mutationFn: async (k: CredentialView) =>
      data(await api.POST("/platform/v1/orgs/{org}/credentials/{credential}/default", { params: { path: { org, credential: k.id } } })),
    invalidates: [["orgs", org, "credentials"]],
    onSuccess: (k) => toast(t("orgHome.keyMadeDefault", "{name} is the {provider} default", { name: k.name, provider: providers.name(k.provider) })),
  });
  const remove = useApiMutation({
    mutationFn: async (k: CredentialView) => {
      await api.DELETE("/platform/v1/orgs/{org}/credentials/{credential}", { params: { path: { org, credential: k.id } } });
    },
    invalidates: [["orgs", org, "credentials"]],
    onSuccess: (_r, k) => {
      setDeleting(null);
      toast(t("orgHome.keyDeleted", "{title} deleted", { title: keyTitle(k, providers) }));
    },
    onError: () => setDeleting(null),
  });

  const setDefault = makeDefault.mutate;
  const columns = useMemo(() => {
    const menu = (k: CredentialView) => {
      const signIn = k.auth_method === "api_key" ? undefined : providers.get(k.provider)?.account;
      const usage = signIn?.usage_url;
      if (!canChange && !canDelete && !usage) return null;
      return (
        <Menu>
          <Menu.Trigger>
            <IconButton icon={icons.more} aria-label={t("orgHome.actionsForKey", "Actions for {title}", { title: keyTitle(k, providers) })} size="xs" />
          </Menu.Trigger>
          <Menu.Content align="end">
            {canChange && !k.is_default && (
              <Menu.Item icon={icons.home} onClick={() => setDefault(k)}>
                {t("orgHome.makeDefault", "Make default")}
              </Menu.Item>
            )}
            {canChange && (
              <Menu.Item icon={icons.route} onClick={() => setAssigning(k)}>
                {t("orgHome.assignToProjects", "Assign to projects")}
              </Menu.Item>
            )}
            {canChange &&
              (k.auth_method === "api_key" ? (
                <Menu.Item icon={icons.providerKey} onClick={() => setReplacing(k)}>
                  {t("orgHome.replace", "Replace")}
                </Menu.Item>
              ) : (
                <Menu.Item icon={icons.session} onClick={() => setReplacing(k)}>
                  {t("orgHome.signInAgain", "Sign in again")}
                </Menu.Item>
              ))}
            {usage && (
              <Menu.LinkItem icon={icons.usage} href={usage} external>
                {t("orgHome.manageUsage", "Manage usage")}
              </Menu.LinkItem>
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

    const addedBy = (k: CredentialView) => t("orgHome.addedBy", "{name}, {date}", { name: partyName(k.created_by), date: formatDate(k.created_at) });

    return [
      col.custom(
        { id: "provider", header: t("common.provider", "Provider"), size: 136, minWidth: 128, priority: 1, grow: true },
        (r: Row) => r,
        (r: Row) => (
          <span className="flex min-w-0 items-center gap-8 sm:gap-12">
            {r.first ? <ProviderBadge provider={r.provider} name={providers.name(r.provider)} size="lg" /> : <span aria-hidden className="w-28 shrink-0" />}
            <Text weight={r.first ? "medium" : undefined} tone={r.first ? "default" : "tertiary"} truncate>
              {providers.name(r.provider)}
            </Text>
          </span>
        ),
      ),
      col.custom(
        { id: "key", header: t("orgHome.key", "Key"), size: accounts ? ACCOUNT_KEY_WIDTH : KEY_WIDTH, priority: 1, grow: true },
        (r: Row) => r,
        (r: Row) => {
          const defaultBelow = r.key.is_default && hidden.includes("projects");
          const under = [
            defaultBelow ? { id: "default", label: null, value: t("orgHome.default", "Default") } : null,
            r.key.account && providers.get(r.provider)?.account?.experimental
              ? { id: "stage", label: t("orgHome.stage", "Stage"), value: t("orgHome.experimental", "Experimental") }
              : null,
            r.key.account ? { id: "account", label: t("orgHome.account", "Account"), value: r.key.account } : null,
            hidden.includes("projects") ? { id: "projects", label: t("common.projects", "Projects"), value: assignment(r.key) } : null,
            hidden.includes("used") ? { id: "used", label: t("common.lastUsed", "Last used"), value: lastUsed(r.key) } : null,
            hidden.includes("added") ? { id: "added", label: t("orgHome.added", "Added"), value: addedBy(r.key) } : null,
          ].filter((part) => part !== null);
          return (
            <span className="flex min-w-0 flex-col">
              <span className="flex min-w-0 items-center gap-8">
                <Text truncate>{r.key.name}</Text>
                {r.key.account ? (
                  <Badge tone="outline" className="hidden sm:inline-flex">
                    {providers.product(r.provider)}
                  </Badge>
                ) : (
                  <Tooltip content={t("orgHome.masterKeyId", "Encrypted with the master key {id}", { id: r.key.key_id })}>
                    <Code className="hidden sm:inline">{`…${r.key.last4}`}</Code>
                  </Tooltip>
                )}
                {r.key.auth_error === "signed_out" && <Badge tone="warning">{t("orgHome.signedOut", "Signed out")}</Badge>}
                {r.key.auth_error === "no_api_access" && (
                  <Tooltip content={t("orgHome.noApiAccessHint", "This {product} plan has no API access.", { product: providers.product(r.provider) })}>
                    <Badge tone="warning">{t("orgHome.noApiAccess", "No API access")}</Badge>
                  </Tooltip>
                )}
                {r.key.is_default && !defaultBelow && <Badge>{t("orgHome.default", "Default")}</Badge>}
              </span>
              {under.length > 0 &&
                withProjectNames(
                  r.key,
                  <Text variant="small" tone="secondary" truncate>
                    {under.map((part, i) => (
                      <span key={part.id}>
                        {i > 0 && t("orgHome.partSeparator", " · ")}
                        {part.label && <VisuallyHidden>{t("orgHome.foldedLabel", "{label}:", { label: part.label })} </VisuallyHidden>}
                        {part.value}
                      </span>
                    ))}
                  </Text>,
                )}
            </span>
          );
        },
      ),
      col.custom(
        { id: "projects", header: t("common.projects", "Projects"), size: 184, priority: 2 },
        (r: Row) => r.key,
        (k: CredentialView) =>
          withProjectNames(
            k,
            <Text variant="small" tone={k.projects.length > 0 || k.is_default ? "secondary" : "tertiary"} truncate>
              {assignment(k)}
            </Text>,
          ),
      ),
      col.custom(
        { id: "added", header: t("orgHome.added", "Added"), size: 216, priority: 3 },
        (r: Row) => r.key,
        (k: CredentialView) => (
          <Text variant="small" tone="secondary" truncate>
            {addedBy(k)}
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
  }, [accounts, canChange, canDelete, hidden, partyName, providers, setDefault]);

  const deletingProvider = deleting ? providers.name(deleting.provider) : "";
  const successor = deleting?.is_default
    ? keys.filter((k) => k.provider === deleting.provider && k.id !== deleting.id).sort((a, b) => b.created_at.localeCompare(a.created_at))[0]
    : undefined;

  return (
    <>
      <OrgHeader
        org={org}
        page={t("orgHome.providers", "Providers")}
        tabs={tabs}
        actions={
          tab === "custom"
            ? !!custom.data?.length && (
                <Button
                  size="sm"
                  variant="primary"
                  leadingIcon={icons.add}
                  disabled={!canManageCustom}
                  {...(customReason ? { disabledReason: customReason } : {})}
                  onClick={() => setAddingCustom(true)}
                >
                  {customLabel}
                </Button>
              )
            : canView &&
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
      {tab === "custom" ? (
        <AppFrame.Body padding="list">
          <CustomProvidersTab org={org} adding={addingCustom} onAddingChange={setAddingCustom} />
        </AppFrame.Body>
      ) : (
        <>
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
                error={credentials.isError && keys.length === 0 ? t("orgHome.providerKeysFailed", "The provider keys couldn't load") : undefined}
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
                          <Button
                            variant="primary"
                            leadingIcon={icons.add}
                            disabled={!canAdd}
                            {...(addReason ? { disabledReason: addReason } : {})}
                            onClick={() => setAdding(true)}
                          >
                            {addLabel}
                          </Button>
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
        </>
      )}
      {adding && <ProviderKeyDialog org={org} keys={keys} onClose={() => setAdding(false)} />}
      {replacing && <ProviderKeyDialog org={org} keys={keys} replace={replacing} onClose={() => setReplacing(null)} />}
      {renaming && <RenameKeyDialog org={org} credential={renaming} onClose={() => setRenaming(null)} />}
      {assigning && <AssignKeyDialog org={org} credential={assigning} keys={keys} onClose={() => setAssigning(null)} />}
      <AlertDialog
        open={!!deleting}
        onOpenChange={(o) => !o && setDeleting(null)}
        tone="danger"
        title={
          deleting
            ? t("orgHome.deleteKeyQuestion", "Delete {title}?", { title: keyTitle(deleting, providers) })
            : t("orgHome.deleteThisKey", "Delete this key?")
        }
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
  const providers = useProviders(org);
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
      title={t("orgHome.renameKey", "Rename {title}", { title: keyTitle(credential, providers) })}
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
  const providers = useProviders(org);
  const label = providers.name(credential.provider);
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
          ? t("orgHome.keyUnassigned", "No project is assigned to {title}", { title: keyTitle(k, providers) })
          : t("orgHome.keyAssigned", "{title} assigned to {count, plural, one {# project} other {# projects}}", {
              title: keyTitle(k, providers),
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
      title={t("orgHome.assignKey", "Assign {title}", { title: keyTitle(credential, providers) })}
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
