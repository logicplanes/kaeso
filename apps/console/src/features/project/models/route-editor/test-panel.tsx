import { useMemo, type ReactNode } from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { Banner, Button, IconButton, Input, Segmented, Select, StatusCode, Text, Timeline, icons } from "@logicplanes/kaeso-ui";
import { FlowWorkspace } from "@logicplanes/kaeso-ui/flow-canvas";
import { formatDuration, formatNumber } from "@logicplanes/kaeso-ui/format";
import { Composer, ResponseMetrics, ResponseView } from "@logicplanes/kaeso-ui/playground";
import { api, data } from "~/api/client";
import type { CatalogueModelView, KeyView, RouteFeature, RouteFormat, RouteStep, RouteTraceRequest, TraceStep } from "~/api/generated/types";
import { translateMessage } from "~/api/gateway-errors";
import { dialectLabel } from "~/format/labels";
import { t } from "~/i18n";
import { metricsOf } from "../../playground/metrics";
import { replyView } from "../../playground/responses";
import { freshId, fromRoute, headerProblem, traced, walk, type DraftStep, type Trace } from "./draft";
import { passed, type Passed, type Sent } from "./send";
import { conditionTitle, featureLabel, headerHint, splitTitle, weightLabel, type Names } from "./labels";
import { glyph } from "./tree";

export type TestHeader = { id: string; name: string; value: string };
export type TestMode = "send" | "preview";

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

export const sentHeaders = (request: TestRequest) =>
  request.headers.filter((h) => h.name.trim() && !headerProblem(h.name)).map((h) => ({ name: h.name.trim(), value: h.value }));

const BYTES_PER_TOKEN = 4;

export function asSent(request: TestRequest, prompt: string, member: string): TestRequest & { member: string } {
  const bytes = new TextEncoder().encode(JSON.stringify({ model: "", input: prompt, stream: true })).length;
  return { ...request, format: "openai_responses", features: ["streaming"], tokens: Math.ceil(bytes / BYTES_PER_TOKEN), key: null, member };
}

function body(route: RouteStep, alias: string | undefined, request: TestRequest & { member?: string }, keys: KeyView[] | undefined): RouteTraceRequest {
  const owner = keys?.find((k) => k.id === request.key)?.principal;
  const by = (kind: string) => (owner?.kind === kind && owner.id) || null;
  return {
    route,
    alias: alias ?? null,
    request: {
      headers: sentHeaders(request),
      format: request.format,
      features: request.features,
      input_tokens: request.tokens,
      key: request.key,
      member: request.member ?? by("user"),
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
  request: TestRequest & { member?: string };
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

function taken(p: Extract<Passed, { kind: "decision" }>): string {
  if (p.step.type === "condition") return p.taken.startsWith("then") ? t("models.route.sent.then", "Takes Then") : t("models.route.sent.else", "Takes Else");
  const index = Number(p.taken.split(".")[1]);
  const weight = p.step.branches.reduce((w, b, i) => (i === index ? b.weight : w), 0);
  return t("models.route.sent.branch", "Takes branch {n}, {weight}", { n: formatNumber(index + 1), weight: weightLabel(weight) });
}

function tried(p: Extract<Passed, { kind: "tried" }>): string {
  const { status, ms } = p.walked;
  if (status === null) return t("models.route.sent.notYours", "Skipped; you may not use this model");
  if (status >= 200 && status < 300) return t("models.route.sent.answered", "Answered");
  const after = ms === null ? null : formatDuration(ms);
  if (p.last) return after ? t("models.route.sent.failedAfter", "Failed after {duration}", { duration: after }) : t("models.route.sent.failed", "Failed");
  return after
    ? t("models.route.sent.fellBackAfter", "Failed after {duration}; fell back", { duration: after })
    : t("models.route.sent.fellBack", "Failed; fell back");
}

function HeaderRows({ request, onRequest }: { request: TestRequest; onRequest: (change: (r: TestRequest) => TestRequest) => void }) {
  const setHeader = (id: string, change: Partial<TestHeader>) =>
    onRequest((r) => ({ ...r, headers: r.headers.map((h) => (h.id === id ? { ...h, ...change } : h)) }));
  return (
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
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-8">
      <Text variant="small" weight="medium">
        {title}
      </Text>
      {children}
    </div>
  );
}

function SentResult({
  sent,
  models,
  providerName,
  title,
  onOpenLog,
}: {
  sent: Sent;
  models: ReadonlyMap<string, CatalogueModelView>;
  providerName: (id: string) => string;
  title: (step: DraftStep) => string;
  onOpenLog: ((id: string) => void) | undefined;
}) {
  const root = useMemo(() => fromRoute(sent.route), [sent.route]);
  const steps = sent.walked ? passed(root, sent.walked) : null;
  const r = sent.response;
  const log = sent.requestId;
  return (
    <>
      {steps?.length !== 0 && (
        <Section title={t("models.route.stepsTook", "Steps it took")}>
          {steps ? (
            <Timeline
              events={steps.map((p) => ({
                id: p.path,
                marker: glyph(p.step, models, providerName),
                title: title(p.step),
                ...(p.kind === "tried" && p.walked.status !== null ? { time: <StatusCode code={p.walked.status} /> } : {}),
                description: p.kind === "decision" ? taken(p) : tried(p),
              }))}
            />
          ) : (
            <Text variant="small" tone="secondary">
              {t("models.route.sent.waiting", "On its way along the route")}
            </Text>
          )}
        </Section>
      )}
      <Section title={t("models.route.reply", "Reply")}>
        <ResponseView
          aria-label={t("models.route.replyLabel", "Reply to the test request")}
          {...replyView(r)}
          footer={<ResponseMetrics {...metricsOf(r, null)} />}
        />
      </Section>
      {log && onOpenLog && (
        <Button variant="secondary" leadingIcon={icons.requestLog} onClick={() => onOpenLog(log)}>
          {t("models.route.openLog", "Open in request log")}
        </Button>
      )}
    </>
  );
}

export function TestPanel({
  mode,
  onMode,
  request,
  onRequest,
  prompt,
  onPrompt,
  root,
  blocked,
  sendBlock,
  trace,
  error,
  loading,
  sent,
  busy,
  onSend,
  onStop,
  onOpenLog,
  keys,
  models,
  names,
  providerName,
  stepName,
}: {
  mode: TestMode;
  onMode: (mode: TestMode) => void;
  request: TestRequest;
  onRequest: (change: (r: TestRequest) => TestRequest) => void;
  prompt: string;
  onPrompt: (prompt: string) => void;
  root: DraftStep;
  blocked: boolean;
  sendBlock: string | undefined;
  trace: Trace | undefined;
  error: { message: string } | null;
  loading: boolean;
  sent: Sent | null;
  busy: boolean;
  onSend: (prompt: string) => void;
  onStop: () => void;
  onOpenLog: ((id: string) => void) | undefined;
  keys: KeyView[] | undefined;
  models: ReadonlyMap<string, CatalogueModelView>;
  names: Names;
  providerName: (id: string) => string;
  stepName: (step: DraftStep) => string;
}) {
  const title = (step: DraftStep) => (step.type === "condition" ? conditionTitle(step, names) : step.type === "split" ? splitTitle(step) : stepName(step));
  const sending = mode === "send";
  return (
    <FlowWorkspace.Panel
      title={t("models.route.test", "Test a request")}
      description={
        sending
          ? t("models.route.sendHint", "Send a request through this route as you; the log marks it Playground.")
          : t("models.route.testHint", "Describe a request to see the steps it takes; nothing is sent.")
      }
    >
      <Segmented
        aria-label={t("models.route.testMode", "How to test")}
        fullWidth
        value={mode}
        onValueChange={(v) => onMode(v === "preview" ? "preview" : "send")}
        options={[
          { value: "send", label: t("models.route.sendMode", "Send") },
          { value: "preview", label: t("models.route.previewMode", "Preview") },
        ]}
      />
      {sending ? (
        <>
          {(blocked || sendBlock) && (
            <Banner tone="warning">{blocked ? t("models.route.needsFixes", "Fix the steps marked on the route first.") : sendBlock}</Banner>
          )}
          <Composer
            aria-label={t("models.route.message", "Message")}
            placeholder={t("models.route.messageHint", "Write a message to send through the route")}
            value={prompt}
            onValueChange={onPrompt}
            onSubmit={onSend}
            clearOnSubmit={false}
            streaming={busy}
            onStop={onStop}
            disabled={blocked || !!sendBlock}
          />
          <HeaderRows request={request} onRequest={onRequest} />
          {sent && <SentResult sent={sent} models={models} providerName={providerName} title={title} onOpenLog={onOpenLog} />}
        </>
      ) : (
        <>
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
          <HeaderRows request={request} onRequest={onRequest} />
          {blocked ? (
            <Banner tone="warning">{t("models.route.needsFixes", "Fix the steps marked on the route first.")}</Banner>
          ) : error ? (
            <Banner tone="danger">{translateMessage(error.message)}</Banner>
          ) : (
            <Section title={t("models.route.stepsTaken", "Steps it takes")}>
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
            </Section>
          )}
        </>
      )}
    </FlowWorkspace.Panel>
  );
}
