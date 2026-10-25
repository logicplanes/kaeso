import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  AlertDialog,
  Badge,
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
  Switch,
  Text,
  Tooltip,
  VisuallyHidden,
  col,
  icons,
} from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import { fieldError } from "~/api/field-error";
import type { CustomAuthView, CustomProviderView, EmbeddingsDialect, GenerationDialect } from "~/api/generated/types";
import { customProvidersQuery } from "~/api/queries/custom-providers";
import { modelCatalogues } from "~/api/queries/invalidation";
import { useApiMutation } from "~/auth/mutation";
import { useNeedsRight } from "~/auth/needs-right";
import { useOrgRights } from "~/auth/session";
import { docsPage } from "~/format/docs";
import { dialectLabel } from "~/format/labels";
import { toast } from "~/shell/toast";
import { t } from "~/i18n";

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
                  {t("orgHome.custom.about", "Read about custom providers")}
                </Button>
              </>
            }
          />
        }
      />
      {adding && <CustomProviderDialog org={org} onClose={() => onAddingChange(false)} />}
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

function draftOf(p: CustomProviderView | undefined): Draft {
  return {
    name: p?.name ?? "",
    id: p?.id ?? "",
    idTouched: !!p,
    base_url: p?.base_url ?? "",
    auth: p?.auth ?? "bearer",
    generation: p?.generation ?? "openai_chat",
    embeddings: !!p?.embeddings,
    count_tokens: !!p?.count_tokens,
  };
}

function draftProblem(d: Draft): string | undefined {
  if (!d.name.trim()) return t("orgHome.custom.nameNeeded", "Give the provider a name");
  if (!/^[a-z0-9]+(-[a-z0-9]+)*$/.test(d.id)) return t("orgHome.custom.idNeeded", "Give the provider an id of lowercase letters, digits and hyphens");
  if (!/^https?:\/\/\S+$/.test(d.base_url.trim())) return t("orgHome.custom.urlNeeded", "Give the base URL, starting with https://");
  return undefined;
}

export function CustomProviderDialog({ org, editing, onClose }: { org: string; editing?: CustomProviderView | undefined; onClose: () => void }) {
  const [draft, setDraft] = useState<Draft>(() => draftOf(editing));
  const set = (patch: Partial<Draft>) => {
    setDraft((d) => ({ ...d, ...patch }));
    if (save.isError) save.reset();
  };
  const counts = COUNTING.has(draft.generation);
  const embeddings: EmbeddingsDialect | null = draft.embeddings ? "openai_embeddings" : null;
  const countTokens: GenerationDialect | null = draft.count_tokens && counts ? draft.generation : null;
  const save = useApiMutation({
    mutationFn: async () => {
      if (editing)
        return data(
          await api.PATCH("/platform/v1/orgs/{org}/custom-providers/{provider}", {
            params: { path: { org, provider: editing.id }, header: { "If-Match": `"${editing.revision}"` } },
            body: {
              name: draft.name.trim(),
              base_url: draft.base_url.trim(),
              auth: draft.auth,
              generation: draft.generation,
              embeddings,
              count_tokens: countTokens,
            },
          }),
        );
      return data(
        await api.POST("/platform/v1/orgs/{org}/custom-providers", {
          params: { path: { org } },
          body: {
            id: draft.id,
            name: draft.name.trim(),
            base_url: draft.base_url.trim(),
            auth: draft.auth,
            generation: draft.generation,
            embeddings,
            count_tokens: countTokens,
          },
        }),
      );
    },
    invalidates: [["orgs", org, "custom-providers"], ["orgs", org, "providers"], modelCatalogues],
    onSuccess: (p) => {
      toast(editing ? t("orgHome.custom.saved", "{name} saved", { name: p.name }) : t("orgHome.custom.added", "{name} added", { name: p.name }));
      onClose();
    },
  });
  const blocked = draftProblem(draft);
  const submit = () => !blocked && save.mutate();
  const keysLeft = editing && draft.auth === "none" && editing.auth !== "none" && editing.in_use.keys > 0;
  return (
    <Dialog
      busy={save.isPending}
      open
      onOpenChange={(o) => !o && onClose()}
      size="md"
      title={editing ? t("orgHome.custom.editTitle", "Edit {name}", { name: editing.name }) : t("orgHome.custom.add", "Add custom provider")}
      description={t("orgHome.custom.dialogHint", "Kaeso calls it in the API format you choose; set each model's prices and limits by hand.")}
      onSubmit={submit}
      footer={
        <>
          <Dialog.Close>
            <Button>{t("common.cancel", "Cancel")}</Button>
          </Dialog.Close>
          <Button variant="primary" disabled={!!blocked} {...(blocked ? { disabledReason: blocked } : {})} loading={save.isPending} onClick={submit}>
            {editing ? t("common.save", "Save") : t("orgHome.custom.add", "Add custom provider")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-20">
        <Input
          label={t("common.name", "Name")}
          value={draft.name}
          onChange={(e) => set({ name: e.target.value, ...(draft.idTouched ? {} : { id: idFromName(e.target.value) }) })}
          error={fieldError(save.error, "name")}
          placeholder={t("orgHome.custom.namePlaceholder", "Research vLLM")}
          maxLength={100}
          autoFocus
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
          error={fieldError(save.error, "id")}
          maxLength={40}
        />
        <Input
          label={t("orgHome.custom.baseUrl", "Base URL")}
          description={t("orgHome.custom.baseUrlHint", "Kaeso adds the format's path, such as /v1/chat/completions, to it.")}
          value={draft.base_url}
          onChange={(e) => set({ base_url: e.target.value })}
          error={fieldError(save.error, "base_url")}
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
          error={fieldError(save.error, "generation")}
        />
        <div className="flex flex-col gap-12">
          <Switch
            label={t("orgHome.custom.embeddings", "Also serves embeddings")}
            description={t("orgHome.custom.embeddingsHint", "In the OpenAI Embeddings format, at /v1/embeddings.")}
            checked={draft.embeddings}
            onCheckedChange={(v) => set({ embeddings: v })}
          />
          {counts && (
            <Switch
              label={t("orgHome.custom.countTokens", "Counts tokens")}
              description={t("orgHome.custom.countTokensHint", "Apps can ask it to count a request's input tokens.")}
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
          error={fieldError(save.error, "auth")}
        />
        {keysLeft && (
          <Text variant="small" tone="secondary">
            {t("orgHome.custom.keysLeft", "Delete its keys under Provider keys first; a provider without a key can't keep any.")}
          </Text>
        )}
      </div>
    </Dialog>
  );
}
