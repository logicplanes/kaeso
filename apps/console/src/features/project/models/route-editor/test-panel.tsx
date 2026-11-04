import { useMemo } from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { Banner, Button, IconButton, Input, Select, Text, Timeline, icons } from "@logicplanes/kaeso-ui";
import { FlowWorkspace } from "@logicplanes/kaeso-ui/flow-canvas";
import { formatNumber } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import type { CatalogueModelView, KeyView, RouteFeature, RouteFormat, RouteStep, RouteTraceRequest, TraceStep } from "~/api/generated/types";
import { translateMessage } from "~/api/gateway-errors";
import { dialectLabel } from "~/format/labels";
import { t } from "~/i18n";
import { freshId, headerProblem, traced, walk, type DraftStep, type Trace } from "./draft";
import { conditionTitle, featureLabel, headerHint, splitTitle, weightLabel, type Names } from "./labels";
import { glyph } from "./tree";

export type TestHeader = { id: string; name: string; value: string };
export type TestRequest = {
  headers: TestHeader[];
  format: RouteFormat;
  features: RouteFeature[];
  tokens: number;
  key: string | null;
  seed: number;
};

const FORMATS: readonly RouteFormat[] = ["openai_chat", "openai_responses", "anthropic_messages", "openai_embeddings"];
const FEATURES: readonly RouteFeature[] = ["images", "pdfs", "tools", "reasoning", "structured_output", "streaming"];
const NO_KEY = "none";
const MAX_HEADERS = 20;
const HEADER_EXAMPLE = "x-plan";
const TOKENS_EXAMPLE = "1000";

export const newSeed = () => Math.floor(Math.random() * 2 ** 32);

export function headersRead(root: DraftStep): string[] {
  const names = new Set<string>();
  for (const { step } of walk(root)) {
    if (step.type === "condition") for (const r of step.rules) if (r.field === "header" && r.name.trim()) names.add(r.name.trim());
    if (step.type === "split" && step.by === "header" && step.header.trim()) names.add(step.header.trim());
  }
  return [...names];
}

export function firstRequest(root: DraftStep): TestRequest {
  return {
    headers: headersRead(root).map((name) => ({ id: freshId("h"), name, value: "" })),
    format: "openai_chat",
    features: [],
    tokens: 1000,
    key: null,
    seed: newSeed(),
  };
}

export const drawsAnew = (root: DraftStep) => [...walk(root)].some(({ step }) => step.type === "split" && step.by !== "caller");

function body(route: RouteStep, alias: string | undefined, request: TestRequest, keys: KeyView[] | undefined): RouteTraceRequest {
  const owner = keys?.find((k) => k.id === request.key)?.principal;
  const by = (kind: string) => (owner?.kind === kind && owner.id) || null;
  return {
    route,
    alias: alias ?? null,
    request: {
      headers: request.headers.filter((h) => h.name.trim() && !headerProblem(h.name)).map((h) => ({ name: h.name.trim(), value: h.value })),
      format: request.format,
      features: request.features,
      input_tokens: request.tokens,
      key: request.key,
      member: by("user"),
      service_account: by("service_account"),
      seed: request.seed,
    },
  };
}

export function useTrace({
  org,
  project,
  alias,
  route,
  root,
  request,
  keys,
  enabled,
}: {
  org: string;
  project: string;
  alias: string | undefined;
  route: RouteStep;
  root: DraftStep;
  request: TestRequest;
  keys: KeyView[] | undefined;
  enabled: boolean;
}) {
  const sent = body(route, alias, request, keys);
  const query = useQuery({
    queryKey: ["orgs", org, "projects", project, "aliases", "trace", sent],
    queryFn: async () => data(await api.POST("/platform/v1/orgs/{org}/projects/{project}/aliases/trace", { params: { path: { org, project } }, body: sent })),
    enabled,
    placeholderData: keepPreviousData,
    staleTime: Infinity,
    retry: false,
    meta: { quiet: true },
  });
  const trace = useMemo(() => (query.data && enabled ? traced(root, query.data.steps) : undefined), [query.data, root, enabled]);
  return { trace, error: query.error, loading: query.isPending && enabled };
}

function outcome(at: TraceStep): string {
  switch (at.type) {
    case "condition": {
      const held = at.rules.filter(Boolean).length;
      const count = at.rules.length;
      if (count === 1) return at.holds ? t("models.route.trace.holds", "Holds; takes Then") : t("models.route.trace.fails", "Doesn't hold; takes Else");
      return at.holds
        ? t("models.route.trace.someThen", "{held} of {count} rules hold; takes Then", { held: formatNumber(held), count: formatNumber(count) })
        : t("models.route.trace.someElse", "{held} of {count} rules hold; takes Else", { held: formatNumber(held), count: formatNumber(count) });
    }
    case "split":
      return t("models.route.trace.drew", "Drew {draw} of 100; takes branch {n}, {weight}", {
        draw: formatNumber(at.draw),
        n: formatNumber(at.branch + 1),
        weight: weightLabel(at.weight),
      });
    case "model":
      return at.attempt === 1
        ? t("models.route.trace.first", "Try 1; tried first")
        : t("models.route.trace.next", "Try {n}; tried when try {previous} fails", {
            n: formatNumber(at.attempt),
            previous: formatNumber(at.attempt - 1),
          });
  }
}

export function TestPanel({
  request,
  onRequest,
  root,
  blocked,
  trace,
  error,
  loading,
  keys,
  models,
  names,
  providerName,
  stepName,
}: {
  request: TestRequest;
  onRequest: (change: (r: TestRequest) => TestRequest) => void;
  root: DraftStep;
  blocked: boolean;
  trace: Trace | undefined;
  error: { message: string } | null;
  loading: boolean;
  keys: KeyView[] | undefined;
  models: ReadonlyMap<string, CatalogueModelView>;
  names: Names;
  providerName: (id: string) => string;
  stepName: (step: DraftStep) => string;
}) {
  const setHeader = (id: string, change: Partial<TestHeader>) =>
    onRequest((r) => ({ ...r, headers: r.headers.map((h) => (h.id === id ? { ...h, ...change } : h)) }));
  const title = (step: DraftStep) => (step.type === "condition" ? conditionTitle(step, names) : step.type === "split" ? splitTitle(step) : stepName(step));
  return (
    <FlowWorkspace.Panel
      title={t("models.route.test", "Test a request")}
      description={t("models.route.testHint", "Describe a request to see the steps it takes; nothing is sent.")}
    >
      <Select
        label={t("models.route.field.format", "API format")}
        fullWidth
        value={request.format}
        onValueChange={(v: string) => onRequest((r) => ({ ...r, format: v as RouteFormat }))}
        options={FORMATS.map((f) => ({ value: f, label: dialectLabel(f) }))}
      />
      <Input
        label={t("models.route.field.inputTokens", "Input tokens")}
        inputMode="numeric"
        value={request.tokens ? String(request.tokens) : ""}
        placeholder={TOKENS_EXAMPLE}
        onChange={(e) => {
          const text = e.target.value.replace(/[^\d]/g, "").slice(0, 8);
          onRequest((r) => ({ ...r, tokens: text === "" ? 0 : Number(text) }));
        }}
      />
      <Select
        label={t("models.route.uses", "Uses")}
        multiple
        fullWidth
        placeholder={t("models.route.usesNothing", "Text only")}
        value={request.features}
        onValueChange={(v: string[]) => onRequest((r) => ({ ...r, features: FEATURES.filter((f) => v.includes(f)) }))}
        options={FEATURES.map((f) => ({ value: f, label: featureLabel[f]() }))}
      />
      <Select
        label={t("models.route.field.key", "API key")}
        fullWidth
        value={request.key ?? NO_KEY}
        onValueChange={(v: string) => onRequest((r) => ({ ...r, key: v === NO_KEY ? null : v }))}
        options={[
          { value: NO_KEY, label: t("models.route.noKey", "No key") },
          ...(keys ?? []).map((k) => ({ value: k.id, label: k.name, description: k.redacted })),
        ]}
      />
      <div className="flex flex-col gap-8">
        <Text variant="small" weight="medium">
          {t("models.route.headers", "Headers")}
        </Text>
        {request.headers.map((h, i) => {
          const problem = h.name.trim() ? headerProblem(h.name) : undefined;
          return (
            <div key={h.id} className="flex items-start gap-8">
              <Input
                aria-label={t("models.route.headerNameN", "Name of header {n}", { n: formatNumber(i + 1) })}
                className="min-w-0 flex-1"
                placeholder={HEADER_EXAMPLE}
                value={h.name}
                error={problem ? headerHint(problem) : undefined}
                onChange={(e) => setHeader(h.id, { name: e.target.value.toLowerCase() })}
              />
              <Input
                aria-label={t("models.route.headerValueN", "Value of header {n}", { n: formatNumber(i + 1) })}
                className="min-w-0 flex-1"
                maxLength={200}
                placeholder={t("models.route.headerValueExample", "paid")}
                value={h.value}
                onChange={(e) => setHeader(h.id, { value: e.target.value })}
              />
              <IconButton
                icon={icons.close}
                variant="ghost"
                aria-label={t("models.route.removeHeaderN", "Remove header {n}", { n: formatNumber(i + 1) })}
                onClick={() => onRequest((r) => ({ ...r, headers: r.headers.filter((x) => x.id !== h.id) }))}
              />
            </div>
          );
        })}
        {request.headers.length < MAX_HEADERS && (
          <Button
            variant="secondary"
            leadingIcon={icons.add}
            onClick={() => onRequest((r) => ({ ...r, headers: [...r.headers, { id: freshId("h"), name: "", value: "" }] }))}
          >
            {t("models.route.addHeader", "Add header")}
          </Button>
        )}
      </div>
      {blocked ? (
        <Banner tone="warning">{t("models.route.needsFixes", "Fix the steps marked on the route first.")}</Banner>
      ) : error ? (
        <Banner tone="danger">{translateMessage(error.message)}</Banner>
      ) : (
        <div className="flex flex-col gap-8">
          <Text variant="small" weight="medium">
            {t("models.route.stepsTaken", "Steps it takes")}
          </Text>
          {loading && !trace ? null : (
            <Timeline
              events={(trace?.steps ?? []).map(({ step, at }) => ({
                id: step.id,
                marker: glyph(step, models, providerName),
                title: title(step),
                description: outcome(at),
              }))}
            />
          )}
          {drawsAnew(root) && (
            <Button variant="secondary" leadingIcon={icons.refresh} onClick={() => onRequest((r) => ({ ...r, seed: newSeed() }))}>
              {t("models.route.drawAgain", "Draw again")}
            </Button>
          )}
        </div>
      )}
    </FlowWorkspace.Panel>
  );
}
