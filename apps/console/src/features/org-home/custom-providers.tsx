import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  AlertDialog,
  Badge,
  Banner,
  Button,
  Code,
  DataTable,
  Dialog,
  Empty,
  IconButton,
  Input,
  Menu,
  ProviderBadge,
  Select,
  SensitiveInput,
  StepDialog,
  Switch,
  Text,
  Tooltip,
  VisuallyHidden,
  col,
  icons,
} from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import { fieldError } from "~/api/field-error";
import type { ApiError } from "~/api/errors";
import type {
  CustomAuthView,
  CustomProviderDetect,
  CustomProviderDetection,
  CustomProviderView,
  EmbeddingsDialect,
  GenerationDialect,
} from "~/api/generated/types";
import { customProvidersQuery } from "~/api/queries/custom-providers";
import { modelCatalogues } from "~/api/queries/invalidation";
import { useApiMutation } from "~/auth/mutation";
import { useNeedsRight } from "~/auth/needs-right";
import { useOrgRights } from "~/auth/session";
import { docsPage } from "~/format/docs";
import { dialectLabel } from "~/format/labels";
import { toast } from "~/shell/toast";
import { locale, t } from "~/i18n";

const GENERATIONS: GenerationDialect[] = ["openai_chat", "openai_responses", "anthropic_messages"];
const COUNTING = new Set<GenerationDialect>(["openai_responses", "anthropic_messages"]);
const AUTHS: CustomAuthView[] = ["bearer", "x_api_key", "none"];

function authLabel(auth: CustomAuthView): string {
  switch (auth) {
    case "bearer":
      return t("orgHome.custom.auth.bearer", "Bearer token");
    case "x_api_key":
      return t("orgHome.custom.auth.xApiKey", "x-api-key header");
    case "none":
      return t("orgHome.custom.auth.none", "No key");
  }
}

function authHint(auth: CustomAuthView): string {
  switch (auth) {
    case "bearer":
      return t("orgHome.custom.auth.bearerHint", "Sent as Authorization: Bearer, as OpenAI-compatible servers expect.");
    case "x_api_key":
      return t("orgHome.custom.auth.xApiKeyHint", "Sent as x-api-key, as Anthropic-compatible servers expect.");
    case "none":
      return t("orgHome.custom.auth.noneHint", "Requests go out without a key, for a server on your own network.");
  }
}

export function idFromName(name: string): string {
  return name
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 40)
    .replace(/-+$/g, "");
}

function usage(p: CustomProviderView): string {
  const { keys, models } = p.in_use;
  if (!keys && !models) return t("orgHome.custom.unused", "Not used yet");
  return t("orgHome.custom.usage", "{keys, plural, one {# key} other {# keys}} · {models, plural, one {# model} other {# models}}", { keys, models });
}

export function deleteConsequence(p: CustomProviderView): string {
  const { keys, models, projects } = p.in_use;
  if (!keys && !models) return t("orgHome.custom.deleteUnused", "Nothing uses it yet; requests are not affected.");
  return t(
    "orgHome.custom.deleteUsed",
    "Its {keys, plural, one {# key} other {# keys}} and {models, plural, one {# model} other {# models}} in {projects, plural, one {# project} other {# projects}} go too; requests to them fail.",
    { keys, models, projects },
  );
}

export function CustomProvidersTab({ org, adding, onAddingChange }: { org: string; adding: boolean; onAddingChange: (adding: boolean) => void }) {
  const rights = useOrgRights(org);
  const needs = useNeedsRight(org);
  const canManage = rights.has("providers.custom.manage");
  const list = useQuery(customProvidersQuery(org));
  const [editing, setEditing] = useState<CustomProviderView | null>(null);
  const [deleting, setDeleting] = useState<CustomProviderView | null>(null);
  const [hidden, setHidden] = useState<string[]>([]);
  const rows = useMemo(() => list.data ?? [], [list.data]);
  const addReason = canManage ? undefined : needs("providers.custom.manage");

  const remove = useApiMutation({
    mutationFn: async (p: CustomProviderView) => {
      await api.DELETE("/platform/v1/orgs/{org}/custom-providers/{provider}", {
        params: { path: { org, provider: p.id }, header: { "If-Match": `"${p.revision}"` } },
      });
    },
    invalidates: [["orgs", org, "custom-providers"], ["orgs", org, "providers"], ["orgs", org, "credentials"], modelCatalogues],
    onSuccess: (_r, p) => {
      setDeleting(null);
      toast(t("orgHome.custom.deleted", "{name} deleted", { name: p.name }));
    },
    onError: () => setDeleting(null),
  });

  const columns = useMemo(() => {
    const menu = (p: CustomProviderView) =>
      canManage ? (
        <Menu>
          <Menu.Trigger>
            <IconButton icon={icons.more} aria-label={t("orgHome.custom.actionsFor", "Actions for {name}", { name: p.name })} size="xs" />
          </Menu.Trigger>
          <Menu.Content align="end">
            <Menu.Item icon={icons.edit} onClick={() => setEditing(p)}>
              {t("common.edit", "Edit")}
            </Menu.Item>
            <Menu.Item icon={icons.delete} variant="danger" onClick={() => setDeleting(p)}>
              {t("common.delete", "Delete")}
            </Menu.Item>
          </Menu.Content>
        </Menu>
      ) : null;
    const folded = (p: CustomProviderView) =>
      [
        { id: "address", label: t("orgHome.custom.baseUrl", "Base URL"), value: p.base_url },
        { id: "format", label: t("orgHome.custom.format", "API format"), value: dialectLabel(p.generation) },
        { id: "use", label: t("orgHome.custom.inUse", "In use"), value: usage(p) },
      ].filter((part) => hidden.includes(part.id));
    return [
      col.custom(
        { id: "provider", header: t("common.provider", "Provider"), size: 240, minWidth: 160, priority: 1, grow: true },
        (p: CustomProviderView) => p,
        (p: CustomProviderView) => (
          <span className="flex min-w-0 items-center gap-8 sm:gap-12">
            <ProviderBadge provider={`custom:${p.id}`} name={p.name} size="lg" />
            <span className="flex min-w-0 flex-col">
              <span className="flex min-w-0 items-center gap-8">
                <Text weight="medium" truncate>
                  {p.name}
                </Text>
                {p.hides_built_in && (
                  <Tooltip
                    content={t("orgHome.custom.hidesHint", "Kaeso now has a built-in {name} with this id; this organization keeps using its own.", {
                      name: p.hides_built_in,
                    })}
                  >
                    <Badge tone="warning">{t("orgHome.custom.hides", "Replaces {name}", { name: p.hides_built_in })}</Badge>
                  </Tooltip>
                )}
              </span>
              <Text variant="small" tone="secondary" truncate>
                <Code>{p.id}</Code>
                {folded(p).map((part) => (
                  <span key={part.id}>
                    {t("orgHome.partSeparator", " · ")}
                    <VisuallyHidden>{t("orgHome.foldedLabel", "{label}:", { label: part.label })} </VisuallyHidden>
                    {part.value}
                  </span>
                ))}
              </Text>
            </span>
          </span>
        ),
      ),
      col.custom(
        { id: "address", header: t("orgHome.custom.baseUrl", "Base URL"), size: 280, priority: 2 },
        (p: CustomProviderView) => p,
        (p: CustomProviderView) => (
          <Text variant="small" tone="secondary" truncate>
            {p.base_url}
          </Text>
        ),
      ),
      col.custom(
        { id: "format", header: t("orgHome.custom.format", "API format"), size: 200, priority: 2 },
        (p: CustomProviderView) => p,
        (p: CustomProviderView) => (
          <Text variant="small" tone="secondary" truncate>
            {dialectLabel(p.generation)}
          </Text>
        ),
      ),
      col.custom(
        { id: "use", header: t("orgHome.custom.inUse", "In use"), size: 160, priority: 3 },
        (p: CustomProviderView) => p,
        (p: CustomProviderView) => (
          <Text variant="small" tone={p.in_use.keys || p.in_use.models ? "secondary" : "tertiary"} truncate>
            {usage(p)}
          </Text>
        ),
      ),
      col.custom(
        { id: "actions", header: "", size: 56, priority: 1 },
        (p: CustomProviderView) => p,
        (p: CustomProviderView) => menu(p),
        { align: "end" },
      ),
    ].map((c) => ({ ...c, enableSorting: false }));
  }, [canManage, hidden]);

  return (
    <>
      <DataTable
        aria-label={t("orgHome.customProviders", "Custom providers")}
        data={rows}
        columns={columns}
        getRowId={(p) => p.id}
        onHiddenColumnsChange={setHidden}
        density="comfortable"
        loading={list.isPending}
        error={list.isError && rows.length === 0 ? t("orgHome.custom.failed", "The custom providers couldn't load") : undefined}
        onRetry={() => void list.refetch()}
        empty={
          <Empty
            title={t("orgHome.custom.empty", "No custom providers yet")}
            description={t("orgHome.custom.emptyHint", "Add a self-hosted server or an internal proxy that speaks an API format Kaeso knows.")}
            actions={
              <>
                <Button
                  variant="primary"
                  leadingIcon={icons.add}
                  disabled={!canManage}
                  {...(addReason ? { disabledReason: addReason } : {})}
                  onClick={() => onAddingChange(true)}
                >
                  {t("orgHome.custom.add", "Add custom provider")}
                </Button>
                <Button variant="ghost" href={docsPage("/console/custom-providers")}>
                  {t("orgHome.custom.about", "About custom providers")}
                </Button>
              </>
            }
          />
        }
      />
      {adding && <AddCustomProviderDialog org={org} onClose={() => onAddingChange(false)} />}
      {editing && <CustomProviderDialog org={org} editing={editing} onClose={() => setEditing(null)} />}
      <AlertDialog
        open={!!deleting}
        onOpenChange={(o) => !o && setDeleting(null)}
        tone="danger"
        title={
          deleting ? t("orgHome.custom.deleteQuestion", "Delete {name}?", { name: deleting.name }) : t("orgHome.custom.deleteThis", "Delete this provider?")
        }
        description={deleting ? deleteConsequence(deleting) : ""}
        confirmLabel={t("orgHome.custom.delete", "Delete provider")}
        loading={remove.isPending}
        onConfirm={() => (deleting ? remove.mutateAsync(deleting) : undefined)}
      />
    </>
  );
}

interface Draft {
  name: string;
  id: string;
  idTouched: boolean;
  base_url: string;
  auth: CustomAuthView;
  generation: GenerationDialect;
  embeddings: boolean;
  count_tokens: boolean;
}

function draftOf(p: CustomProviderView | CustomProviderDetection, idTouched: boolean): Draft {
  return {
    name: p.name,
    id: p.id,
    idTouched,
    base_url: p.base_url,
    auth: p.auth,
    generation: p.generation,
    embeddings: !!p.embeddings,
    count_tokens: !!p.count_tokens,
  };
}

function draftProblem(d: Draft): string | undefined {
  if (!d.name.trim()) return t("orgHome.custom.nameNeeded", "Give the provider a name");
  if (!/^[a-z0-9]+(-[a-z0-9]+)*$/.test(d.id)) return t("orgHome.custom.idNeeded", "Give the provider an id of lowercase letters, digits and hyphens");
  if (!/^https?:\/\/\S+$/.test(d.base_url.trim())) return t("orgHome.custom.urlNeeded", "Give the base URL, starting with https://");
  return undefined;
}

function settingsOf(d: Draft) {
  const embeddings: EmbeddingsDialect | null = d.embeddings ? "openai_embeddings" : null;
  const count_tokens: GenerationDialect | null = d.count_tokens && COUNTING.has(d.generation) ? d.generation : null;
  return { base_url: d.base_url.trim(), auth: d.auth, generation: d.generation, embeddings, count_tokens };
}

function SettingsFields({
  draft,
  set,
  error,
  editing,
  autoFocus,
}: {
  draft: Draft;
  set: (patch: Partial<Draft>) => void;
  error: ApiError | null;
  editing?: CustomProviderView | undefined;
  autoFocus?: boolean | undefined;
}) {
  const counts = COUNTING.has(draft.generation);
  const keysLeft = editing && draft.auth === "none" && editing.auth !== "none" && editing.in_use.keys > 0;
  return (
    <div className="flex flex-col gap-20">
      <Input
        label={t("common.name", "Name")}
        value={draft.name}
        onChange={(e) => set({ name: e.target.value, ...(draft.idTouched ? {} : { id: idFromName(e.target.value) }) })}
        error={fieldError(error, "name")}
        placeholder={t("orgHome.custom.namePlaceholder", "Research vLLM")}
        maxLength={100}
        autoFocus={autoFocus}
      />
      <Input
        label={t("orgHome.custom.id", "Id")}
        description={
          editing
            ? t("orgHome.custom.idFixed", "The id never changes; model names start with it.")
            : t("orgHome.custom.idHint", "Model names start with it, such as {id}/llama-3.3-70b.", { id: draft.id || "research-vllm" })
        }
        value={draft.id}
        disabled={!!editing}
        onChange={(e) => set({ id: e.target.value.toLowerCase(), idTouched: true })}
        error={fieldError(error, "id")}
        maxLength={40}
      />
      <Input
        label={t("orgHome.custom.baseUrl", "Base URL")}
        description={t("orgHome.custom.baseUrlHint", "Kaeso adds the format's path, such as /v1/chat/completions, to it.")}
        value={draft.base_url}
        onChange={(e) => set({ base_url: e.target.value })}
        error={fieldError(error, "base_url")}
        placeholder={t("orgHome.custom.baseUrlPlaceholder", "https://llm.example.com")}
        maxLength={2048}
        inputMode="url"
      />
      <Select
        label={t("orgHome.custom.format", "API format")}
        description={t("orgHome.custom.formatHint", "vLLM, Ollama, LM Studio, TGI and SGLang speak OpenAI Chat Completions.")}
        fullWidth
        value={draft.generation}
        onValueChange={(v) => set({ generation: v as GenerationDialect })}
        options={GENERATIONS.map((g) => ({ value: g, label: dialectLabel(g) }))}
        error={fieldError(error, "generation")}
      />
      <div className="flex flex-col gap-12">
        <Switch
          label={t("orgHome.custom.embeddings", "Also serves embeddings")}
          description={fieldError(error, "embeddings") ?? t("orgHome.custom.embeddingsHint", "In the OpenAI Embeddings format, at /v1/embeddings.")}
          checked={draft.embeddings}
          onCheckedChange={(v) => set({ embeddings: v })}
        />
        {counts && (
          <Switch
            label={t("orgHome.custom.countTokens", "Counts tokens")}
            description={fieldError(error, "count_tokens") ?? t("orgHome.custom.countTokensHint", "Apps can ask it to count a request's input tokens.")}
            checked={draft.count_tokens}
            onCheckedChange={(v) => set({ count_tokens: v })}
          />
        )}
      </div>
      <Select
        label={t("orgHome.custom.auth", "Key")}
        description={authHint(draft.auth)}
        fullWidth
        value={draft.auth}
        onValueChange={(v) => set({ auth: v as CustomAuthView })}
        options={AUTHS.map((a) => ({ value: a, label: authLabel(a) }))}
        error={fieldError(error, "auth")}
      />
      {keysLeft && (
        <Text variant="small" tone="secondary">
          {t("orgHome.custom.keysLeft", "Delete its keys under Provider keys first; a provider without a key can't keep any.")}
        </Text>
      )}
    </div>
  );
}

const SHOWN_MODELS = 4;

function modelsFound(found: CustomProviderDetection): string {
  if (!found.models) return t("orgHome.custom.noListing", "The server has no model listing; type each model's id when you add it to a project.");
  const count = found.models.length;
  if (!count) return t("orgHome.custom.noModels", "The server lists no models yet.");
  const shown = found.models.slice(0, SHOWN_MODELS).map((m) => m.model_id);
  const names = new Intl.ListFormat(locale(), { type: "unit" }).format(
    count > SHOWN_MODELS ? [...shown, t("orgHome.custom.moreModels", "{count} more", { count: count - SHOWN_MODELS })] : shown,
  );
  return t("orgHome.custom.models", "The server lists {count, plural, one {# model} other {# models}}: {names}.", { count, names });
}

const NOTHING_FOUND: CustomProviderDetection = {
  base_url: "",
  name: "",
  id: "",
  auth: "bearer",
  generation: "openai_chat",
  embeddings: null,
  count_tokens: null,
  models: null,
};

function changedFrom(found: CustomProviderDetection, d: Draft): boolean {
  const s = settingsOf(d);
  return (
    s.base_url !== found.base_url ||
    s.auth !== found.auth ||
    s.generation !== found.generation ||
    s.embeddings !== found.embeddings ||
    s.count_tokens !== found.count_tokens
  );
}

export function AddCustomProviderDialog({ org, onClose }: { org: string; onClose: () => void }) {
  const rights = useOrgRights(org);
  const savesKey = rights.has("provider_credentials.create");
  const [server, setServer] = useState({ base_url: "", key: "" });
  const [found, setFound] = useState<CustomProviderDetection>(NOTHING_FOUND);
  const [draft, setDraft] = useState<Draft>(() => draftOf(NOTHING_FOUND, false));
  const detect = async (body: CustomProviderDetect) =>
    data(await api.POST("/platform/v1/orgs/{org}/custom-provider-detection", { params: { path: { org } }, body }));
  const detection = useApiMutation({ mutationFn: detect });
  const save = useApiMutation({
    mutationFn: async (): Promise<{ added: CustomProviderView; keySaved: boolean }> => {
      const key = server.key.trim();
      const settings = settingsOf(draft);
      if (changedFrom(found, draft)) await detect({ ...settings, key: key || null });
      const added = data(
        await api.POST("/platform/v1/orgs/{org}/custom-providers", {
          params: { path: { org } },
          body: { id: draft.id, name: draft.name.trim(), ...settings },
        }),
      );
      if (!key || draft.auth === "none" || !savesKey) return { added, keySaved: false };
      const keySaved = await api
        .POST("/platform/v1/orgs/{org}/credentials", { params: { path: { org } }, body: { provider: added.id, name: added.name, secret: key } })
        .then(
          () => true,
          () => false,
        );
      return { added, keySaved };
    },
    invalidates: [["orgs", org, "custom-providers"], ["orgs", org, "providers"], ["orgs", org, "credentials"], modelCatalogues],
    onSuccess: ({ added, keySaved }) => {
      if (server.key.trim() && draft.auth !== "none" && savesKey && !keySaved)
        toast.error(t("orgHome.custom.keyNotSaved", "{name} added without its key; add it under Provider keys.", { name: added.name }));
      else toast(t("orgHome.custom.added", "{name} added", { name: added.name }));
      onClose();
    },
  });
  const changeServer = (patch: Partial<typeof server>) => {
    setServer((s) => ({ ...s, ...patch }));
    if (detection.isError) detection.reset();
  };
  const set = (patch: Partial<Draft>) => {
    setDraft((d) => ({ ...d, ...patch }));
    if (save.isError) save.reset();
  };
  const runDetection = async () => {
    try {
      const d = await detection.mutateAsync({ base_url: server.base_url.trim(), key: server.key.trim() || null });
      setFound(d);
      setDraft(draftOf(d, false));
      return true;
    } catch {
      return false;
    }
  };
  return (
    <StepDialog
      open
      onOpenChange={(o) => !o && onClose()}
      size="md"
      title={t("orgHome.custom.add", "Add custom provider")}
      submitLabel={t("orgHome.custom.add", "Add custom provider")}
      submitting={save.isPending}
      onSubmit={() => !draftProblem(draft) && save.mutate()}
      steps={[
        {
          id: "server",
          title: t("orgHome.custom.serverTitle", "Where is the server?"),
          description: t("orgHome.custom.serverHint", "Kaeso asks it for its format, key style and models; nothing is saved yet."),
          blockedReason: server.base_url.trim() ? undefined : t("orgHome.custom.serverNeeded", "Give the server's base URL"),
          continueLabel: t("orgHome.custom.detect", "Detect settings"),
          onContinue: runDetection,
          content: (
            <div className="flex flex-col gap-20">
              <Input
                label={t("orgHome.custom.baseUrl", "Base URL")}
                description={t("orgHome.custom.serverUrlHint", "The address you'd give an SDK; Kaeso drops /v1 and endpoint paths.")}
                value={server.base_url}
                onChange={(e) => changeServer({ base_url: e.target.value })}
                error={fieldError(detection.error, "base_url")}
                placeholder={t("orgHome.custom.serverUrlPlaceholder", "https://llm.example.com/v1")}
                maxLength={2048}
                inputMode="url"
                autoFocus
              />
              <SensitiveInput
                label={t("orgHome.custom.key", "API key")}
                description={
                  savesKey
                    ? t("orgHome.custom.keyHint", "Leave it empty for a server without one; it's saved as the provider's key.")
                    : t("orgHome.custom.keyOnlyDetects", "Leave it empty for a server without one; it's used only to detect settings.")
                }
                value={server.key}
                onChange={(e) => changeServer({ key: e.target.value })}
                error={fieldError(detection.error, "key")}
                autoComplete="off"
              />
            </div>
          ),
        },
        {
          id: "settings",
          title: t("orgHome.custom.foundTitle", "Check what Kaeso found"),
          description: t("orgHome.custom.foundHint", "Change anything that's wrong; Kaeso checks your changes with the server before adding it."),
          blockedReason: draftProblem(draft),
          content: (
            <div className="flex flex-col gap-20">
              <Banner tone="info">{modelsFound(found)}</Banner>
              <SettingsFields draft={draft} set={set} error={save.error} />
            </div>
          ),
        },
      ]}
    />
  );
}

export function CustomProviderDialog({ org, editing, onClose }: { org: string; editing: CustomProviderView; onClose: () => void }) {
  const [draft, setDraft] = useState<Draft>(() => draftOf(editing, true));
  const set = (patch: Partial<Draft>) => {
    setDraft((d) => ({ ...d, ...patch }));
    if (save.isError) save.reset();
  };
  const save = useApiMutation({
    mutationFn: async () => {
      const { base_url, auth, generation, embeddings, count_tokens } = settingsOf(draft);
      return data(
        await api.PATCH("/platform/v1/orgs/{org}/custom-providers/{provider}", {
          params: { path: { org, provider: editing.id }, header: { "If-Match": `"${editing.revision}"` } },
          body: { name: draft.name.trim(), base_url, auth, generation, embeddings, count_tokens },
        }),
      );
    },
    invalidates: [["orgs", org, "custom-providers"], ["orgs", org, "providers"], modelCatalogues],
    onSuccess: (p) => {
      toast(t("orgHome.custom.saved", "{name} saved", { name: p.name }));
      onClose();
    },
  });
  const blocked = draftProblem(draft);
  const submit = () => !blocked && save.mutate();
  return (
    <Dialog
      busy={save.isPending}
      open
      onOpenChange={(o) => !o && onClose()}
      size="md"
      title={t("orgHome.custom.editTitle", "Edit {name}", { name: editing.name })}
      description={t("orgHome.custom.editHint", "Kaeso calls it with these settings; a new address, key style or format marks its models unchecked.")}
      onSubmit={submit}
      footer={
        <>
          <Dialog.Close>
            <Button>{t("common.cancel", "Cancel")}</Button>
          </Dialog.Close>
          <Button variant="primary" disabled={!!blocked} {...(blocked ? { disabledReason: blocked } : {})} loading={save.isPending} onClick={submit}>
            {t("common.save", "Save")}
          </Button>
        </>
      }
    >
      <SettingsFields draft={draft} set={set} error={save.error} editing={editing} autoFocus />
    </Dialog>
  );
}
