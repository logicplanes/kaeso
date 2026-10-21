import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Badge,
  Banner,
  Button,
  DialogScope,
  Empty,
  Input,
  LayerDialog,
  List,
  ModelLabel,
  ProviderPicker,
  SearchInput,
  Switch,
  Text,
} from "@logicplanes/kaeso-ui";
import { formatNumber } from "@logicplanes/kaeso-ui/format";
import { toast } from "~/shell/toast";
import { api, data } from "~/api/client";
import { toastApiError } from "~/api/toast-error";
import type { BatchResult, Fields, Preview, SearchHit } from "~/api/generated/types";
import { currentModelsQuery, modelPreviewQuery, modelSearchQuery } from "~/api/queries/models";
import { useApiMutation } from "~/auth/mutation";
import { useOrgProviderKeys, useProviderOptions, useProviders } from "~/kaeso/providers";
import { orgPath } from "~/shell/pages";
import { recall, remember } from "~/shell/session-memory";
import { monthLabel } from "~/format/period";
import { ModelForm, allFields, changedFields, draftFrom, draftProblem, type FieldDraft } from "./model-form";
import { contextLabel, priceLabel } from "./model-fields";
import { t } from "~/i18n";
import { tRich } from "~/i18n/rich";
import { common } from "~/i18n/common";

type View = "search" | "review" | "custom" | "all";

const LAST_PROVIDER = ["add-models", "provider"];
const MANY_MODELS = 50;

function useDebounced<T>(value: T, ms: number): T {
  const [v, setV] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => setV(value), ms);
    return () => clearTimeout(timer);
  }, [value, ms]);
  return v;
}

interface AddModelsProps {
  org: string;
  project: string;
  open: boolean;
  onClose: () => void;
}

export function AddModelsDialog(props: AddModelsProps) {
  return (
    <DialogScope open={props.open}>
      <AddModels {...props} />
    </DialogScope>
  );
}

function AddModels({ org, project, open, onClose: close }: AddModelsProps) {
  const queryClient = useQueryClient();
  const [view, setView] = useState<View>("search");
  const catalog = useProviders(org);
  const [chosen, setProvider] = useState(() => recall<string>(LAST_PROVIDER));
  const provider = (chosen && catalog.get(chosen) ? chosen : catalog.list[0]?.id) ?? "";
  const providers = useProviderOptions(org);
  const dialects = catalog.dialects(provider);
  const orgKeys = useOrgProviderKeys(org);
  const noKey = orgKeys.known && !orgKeys.keys.some((k) => k.provider === provider);
  const [q, setQ] = useState("");
  const [deprecated, setDeprecated] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [review, setReview] = useState<{ preview: Preview; draft: FieldDraft } | null>(null);
  const [custom, setCustom] = useState<{ modelId: string; preview: Preview | null; checked: boolean; draft: FieldDraft }>(emptyCustom());
  const search = useDebounced(q.trim(), 250);
  const hits = useQuery({ ...modelSearchQuery(org, project, { q: search, provider, include_deprecated: deprecated }), enabled: open && !!provider });
  const current = useQuery({ ...currentModelsQuery(org, project, provider), enabled: open && !!provider });
  const addable = current.data?.not_in_catalogue ?? 0;

  const invalidates = [["orgs", org, "projects", project, "models"]];

  const batch = useApiMutation({
    mutationFn: async (body: { all_current: true } | { model_ids: string[] }): Promise<BatchResult> =>
      data(await api.POST("/platform/v1/orgs/{org}/projects/{project}/models/batch", { params: { path: { org, project } }, body: { provider, ...body } })),
    invalidates,
    onSuccess: (r) => {
      toast(
        t("models.addedCount", "Added {count, plural, one {# model} other {# models}}", { count: r.added.length }),
        r.skipped.length ? { description: t("models.skippedCount", "{count} already in the catalogue", { count: r.skipped.length }) } : undefined,
      );
      close();
    },
  });
  const addOne = useApiMutation({
    mutationFn: async (body: { model_id: string; custom: boolean; overrides: Partial<Fields> }) =>
      data(
        await api.POST("/platform/v1/orgs/{org}/projects/{project}/models", {
          params: { path: { org, project } },
          body: { provider, model_id: body.model_id, custom: body.custom, overrides: body.overrides },
        }),
      ),
    invalidates,
    onSuccess: (m) => {
      toast(t("models.added", "Added {model}", { model: m.model_id }));
      close();
    },
  });

  const startReview = async (modelId: string) => {
    try {
      const preview = await queryClient.fetchQuery(modelPreviewQuery(org, project, provider, modelId));
      if (!preview) return toast.error(t("models.noLongerOnModelsDev", "models.dev doesn't list {model} any more.", { model: modelId }));
      setReview({ preview, draft: draftFrom(previewFields(preview)) });
      setView("review");
    } catch (e) {
      toastApiError(e);
    }
  };

  const checkCustom = async () => {
    const modelId = custom.modelId.trim();
    if (!modelId || custom.checked) return;
    const preview = await queryClient.fetchQuery(modelPreviewQuery(org, project, provider, modelId)).catch(() => null);
    setCustom({ modelId, preview, checked: true, draft: preview ? draftFrom(previewFields(preview)) : custom.draft });
  };

  const ids = [...selected];
  const only = ids.length === 1 ? ids[0] : undefined;
  const name = catalog.name(provider);
  const list = hits.data?.data ?? [];
  const total = hits.data?.total ?? 0;

  const addSelected = () => {
    if (ids.length) batch.mutate({ model_ids: ids });
  };
  const reviewProblem = review ? draftProblem(review.draft, false) : null;
  const addReviewed = () => {
    if (review && !reviewProblem)
      addOne.mutate({ model_id: review.preview.model_id, custom: false, overrides: changedFields(review.draft, previewFields(review.preview), dialects) });
  };
  const isCustom = custom.checked && !custom.preview;
  const customProblem = custom.checked ? draftProblem(custom.draft, isCustom) : null;
  const addCustom = () => {
    const modelId = custom.modelId.trim();
    if (!custom.checked) return void checkCustom();
    if (!modelId || customProblem) return;
    addOne.mutate({
      model_id: modelId,
      custom: isCustom,
      overrides: custom.preview ? changedFields(custom.draft, previewFields(custom.preview), dialects) : allFields(custom.draft, dialects),
    });
  };
  const addAll = () => batch.mutate({ all_current: true });

  const searchLayer = {
    title: t("models.addModels", "Add models"),
    description: t("models.addHint", "Search models.dev; each model brings its prices, limits and capabilities, which you can override."),
    content: (
      <div className="flex flex-col gap-16">
        <div className="flex flex-wrap items-center gap-8 sm:flex-nowrap">
          <ProviderPicker
            appearance="select"
            aria-label={common.provider()}
            providers={providers}
            value={provider}
            onValueChange={(p) => {
              if (!p) return;
              remember(LAST_PROVIDER, p);
              setProvider(p);
              setSelected(new Set());
            }}
          />
          <div className="min-w-0 flex-1 basis-200">
            <SearchInput
              aria-label={t("models.search", "Search models")}
              placeholder={common.search()}
              value={q}
              onChange={(e) => setQ(e.target.value)}
              data-autofocus
            />
          </div>
          <Switch size="sm" label={t("models.includeDeprecated", "Include deprecated")} checked={deprecated} onCheckedChange={setDeprecated} />
        </div>
        {noKey && (
          <Banner
            tone="warning"
            actions={
              <Button size="sm" href={`${orgPath(org)}/providers`}>
                {t("models.openProviders", "Open Providers")}
              </Button>
            }
          >
            {t("models.noProviderKey", "Requests to these models fail until someone adds a key for {provider} on the Providers page.", { provider: name })}
          </Banner>
        )}
        <div className="flex min-h-0 flex-col">
          {hits.isPending && provider ? (
            <List aria-label={t("models.onModelsDev", "Models on models.dev")} density="compact" items={[]} loading />
          ) : list.length === 0 ? (
            <Empty
              title={t("models.noneFound", "No models found")}
              description={
                search
                  ? t("models.noneFoundHint", "Try another name, or include deprecated models.")
                  : t("models.noneListed", "models.dev lists no {provider} models.", { provider: name })
              }
            />
          ) : (
            <List
              aria-label={t("models.onModelsDev", "Models on models.dev")}
              density="compact"
              selectable
              selectionBar={false}
              selected={selected}
              onSelectedChange={setSelected}
              onOpen={(item) => void startReview(item.id)}
              items={list.map((h) => ({
                id: h.model_id,
                textValue: h.model_id,
                title: (
                  <ModelLabel
                    provider={h.provider}
                    providerName={catalog.name(h.provider)}
                    model={h.model_id}
                    mark={false}
                    {...(h.name && h.name !== h.model_id ? { name: h.name } : {})}
                  />
                ),
                meta: <HitMeta hit={h} />,
                date: h.release_date ? monthLabel(h.release_date) : undefined,
                disabled: !!h.catalogue_model_id,
              }))}
            />
          )}
        </div>
        {hits.data?.has_more && (
          <Text variant="small" tone="secondary">
            {t("models.showingSome", "Showing {shown} of {total} {provider} models; search to narrow them down.", {
              shown: formatNumber(list.length),
              total: formatNumber(total),
              provider: name,
            })}
          </Text>
        )}
        <Text variant="small" tone="secondary">
          {tRich("models.notOnModelsDev", "Not on models.dev? {link}", {
            link: (
              <Button variant="link" size="sm" onClick={() => setView("custom")}>
                {t("models.addCustom", "Add a custom model")}
              </Button>
            ),
          })}
        </Text>
      </div>
    ),
    footer: (
      <>
        <Button onClick={() => setView("all")}>{t("models.addAllCurrent", "Add every current {provider} model", { provider: name })}</Button>
        {only && <Button onClick={() => void startReview(only)}>{t("models.reviewValues", "Review values")}</Button>}
        <Button variant="primary" disabled={!ids.length} loading={batch.isPending} onClick={addSelected}>
          {t("models.addCount", "{count, plural, =0 {Add models} one {Add # model} other {Add # models}}", { count: ids.length })}
        </Button>
      </>
    ),
  };

  const reviewLayer = {
    title: review ? t("models.reviewModel", "Review {model}", { model: review.preview.model_id }) : t("models.reviewValues", "Review values"),
    description: t("models.reviewHint", "Values from models.dev, completed by Kaeso; what you change becomes an override for this project."),
    content: review ? (
      <div className="flex flex-col gap-16">
        <ModelForm value={review.draft} onChange={(draft) => setReview({ ...review, draft })} dialects={dialects} />
      </div>
    ) : null,
    footer: (
      <Button
        variant="primary"
        disabled={!review || !!reviewProblem}
        {...(reviewProblem ? { disabledReason: reviewProblem } : {})}
        loading={addOne.isPending}
        onClick={addReviewed}
      >
        {t("models.addModel", "Add model")}
      </Button>
    ),
  };

  const customLayer = {
    title: t("models.addCustom", "Add a custom model"),
    description: t("models.addCustomHint", "Give the provider's model id; if models.dev lists it, its values fill in, otherwise enter them."),
    content: (
      <div className="flex flex-col gap-16">
        <Input
          label={t("models.modelId", "Model id")}
          description={t("models.modelIdHint", "The id {provider} uses, sent as `model`.", { provider: name })}
          value={custom.modelId}
          onChange={(e) => setCustom({ ...custom, modelId: e.target.value, checked: false })}
          onBlur={() => void checkCustom()}
          onKeyDown={(e) => e.key === "Enter" && !e.metaKey && !e.ctrlKey && (e.preventDefault(), void checkCustom())}
          maxLength={200}
        />
        {custom.checked &&
          (custom.preview ? (
            <Banner tone="info">{t("models.customListed", "models.dev lists {model}: its values are filled in.", { model: custom.preview.model_id })}</Banner>
          ) : (
            <Banner tone="neutral">
              {t("models.customUnlisted", "models.dev doesn't list it. Inputs, outputs, limits, input and output prices and the format are required.")}
            </Banner>
          ))}
        {custom.checked && <ModelForm value={custom.draft} onChange={(draft) => setCustom({ ...custom, draft })} dialects={dialects} />}
      </div>
    ),
    footer: custom.checked ? (
      <Button
        variant="primary"
        disabled={!custom.modelId.trim() || !!customProblem}
        {...(customProblem ? { disabledReason: customProblem } : {})}
        loading={addOne.isPending}
        onClick={addCustom}
      >
        {t("models.addModel", "Add model")}
      </Button>
    ) : (
      <Button variant="primary" disabled={!custom.modelId.trim()} onClick={addCustom}>
        {common.continue()}
      </Button>
    ),
  };

  const allLayer = {
    title: t("models.addAllCurrent", "Add every current {provider} model", { provider: name }),
    description: t("models.addAllCurrentHint", "Adds each current {provider} text or embedding model from models.dev and skips ones already here.", {
      provider: name,
    }),
    content: (
      <div className="flex flex-col gap-12">
        {addable >= MANY_MODELS && (
          <Banner tone="warning">
            {t("models.addAllWarning", "This adds {count} models at once; adding the ones you need keeps the catalogue short.", {
              count: formatNumber(addable),
            })}
          </Banner>
        )}
        {current.isSuccess && addable < MANY_MODELS && (
          <Text>
            {addable
              ? t(
                  "models.addAllCount",
                  "{count, plural, one {# current model isn't in the catalogue yet.} other {# current models aren't in the catalogue yet.}}",
                  {
                    count: addable,
                  },
                )
              : t("models.addAllNone", "Every current {provider} model is in the catalogue already.", { provider: name })}
          </Text>
        )}
        <Text variant="small" tone="secondary">
          {t("models.addAllCurrentLater", "Models that models.dev adds later aren't added here on their own.")}
        </Text>
      </div>
    ),
    footer: (
      <Button variant="primary" loading={batch.isPending} disabled={current.isSuccess && addable === 0} onClick={addAll}>
        {t("models.addAllCurrent", "Add every current {provider} model", { provider: name })}
      </Button>
    ),
  };

  const layers = { review: reviewLayer, custom: customLayer, all: allLayer };
  const submit = { search: addSelected, review: addReviewed, custom: addCustom, all: addAll };

  return (
    <LayerDialog
      busy={batch.isPending || addOne.isPending}
      open={open}
      onOpenChange={(o) => !o && close()}
      size="lg"
      layer={view === "search" ? 0 : 1}
      onLayerChange={(l) => l === 0 && setView("search")}
      onSubmit={submit[view]}
      layers={view === "search" ? [searchLayer] : [searchLayer, layers[view]]}
    />
  );
}

function HitMeta({ hit }: { hit: SearchHit }) {
  if (hit.catalogue_model_id) return <Badge>{t("models.inCatalogue", "Already in catalogue")}</Badge>;
  return (
    <span className="flex items-center gap-12">
      {hit.deprecated && <Badge tone="warning">{t("models.deprecated", "Deprecated")}</Badge>}
      <Text variant="small" tone="secondary" tabular className="hidden min-w-48 text-right sm:block">
        {contextLabel(hit.context_limit)}
      </Text>
      <Text variant="small" tone="secondary" tabular className="min-w-96 text-right">
        {priceLabel(hit)}
      </Text>
    </span>
  );
}

const previewFields = (p: Preview): Fields => ({ ...p.fields, egress_dialect: p.fields.egress_dialect ?? p.egress_dialect });

function emptyCustom() {
  return { modelId: "", preview: null, checked: false, draft: draftFrom({}) };
}
