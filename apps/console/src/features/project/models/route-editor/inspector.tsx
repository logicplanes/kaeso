import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Banner,
  Button,
  Checkbox,
  CheckboxGroup,
  CodeBlock,
  Code,
  Combobox,
  Input,
  KeyValue,
  Menu,
  Segmented,
  Select,
  Text,
  icons,
} from "@logicplanes/kaeso-ui";
import { FlowWorkspace } from "@logicplanes/kaeso-ui/flow-canvas";
import { ModelPicker, type ModelPickerOption, type ModelPickerProvider } from "@logicplanes/kaeso-ui/playground";
import { formatNumber } from "@logicplanes/kaeso-ui/format";
import type { RouteFailure, RouteFeature, RouteFormat, RouteRule } from "~/api/generated/types";
import { allKeysQuery } from "~/api/queries/keys";
import { projectMembersQuery, serviceAccountsQuery } from "~/api/queries/members";
import { dialectLabel } from "~/format/labels";
import { t } from "~/i18n";
import { tRich } from "~/i18n/rich";
import {
  FAILURES,
  MAX_BRANCHES,
  MAX_RULES,
  MAX_TIMEOUT,
  TEMPLATES,
  defaultRule,
  emptyModel,
  headerProblem,
  shareEvenly,
  freshId,
  removals,
  type DraftCondition,
  type DraftModel,
  type DraftRule,
  type DraftSplit,
  type DraftStep,
  type Problem,
  type Removal,
  type Template,
} from "./draft";
import { failureHint, failureLabel, featureLabel, fieldLabel, headerHint, kindLabel, problemLabel, templateHint, templateLabel, weightLabel } from "./labels";

export type FieldErrors = ReadonlyMap<string, { field: string; message: string }>;

export type InspectorContext = {
  org: string;
  project: string;
  root: DraftStep;
  readOnly: boolean;
  options: ModelPickerOption[];
  providers: ModelPickerProvider[];
  problems: ReadonlyMap<string, Problem[]>;
  errors: FieldErrors;
  stepName: (step: DraftStep) => string;
  onChange: (id: string, change: (step: DraftStep) => DraftStep, coalesce?: string) => void;
  onRemove: (id: string, how: Removal) => void;
  onAddFallback: (id: string) => void;
};

const HEADER_EXAMPLE = "x-plan";
const SPLIT_HEADER_EXAMPLE = "x-user-id";
const MODEL_FIELD = "model";
const SNIPPETS = { curl: "curl", python: "Python", ts: "TypeScript" } as const;

const FORMATS: readonly RouteFormat[] = [
  "openai_chat",
  "openai_responses",
  "anthropic_messages",
  "openai_embeddings",
  "gemini",
  "gemini_embeddings",
  "openai_decisions",
  "system_one",
  "gemini_decisions",
];
const FEATURES: readonly RouteFeature[] = ["images", "pdfs", "tools", "reasoning", "structured_output", "streaming"];

const update = <T extends DraftStep>(ctx: InspectorContext, step: T, change: (s: T) => T, coalesce?: string) =>
  ctx.onChange(step.id, (s) => change(s as T), coalesce);

const fieldError = (ctx: InspectorContext, id: string, field: string) => {
  const e = ctx.errors.get(id);
  return e && (e.field === field || e.field.startsWith(`${field}.`)) ? e.message : undefined;
};

function StepProblems({ ctx, id }: { ctx: InspectorContext; id: string }) {
  const found = ctx.problems.get(id) ?? [];
  const server = ctx.errors.get(id);
  if (!found.length && !server) return null;
  return <Banner tone="warning">{server ? server.message : found.map(problemLabel).join(" · ")}</Banner>;
}

function RemoveMenu({ ctx, step }: { ctx: InspectorContext; step: DraftStep }) {
  const ways = removals(ctx.root, step.id);
  if (ctx.readOnly || ways.length === 0) return null;
  const label = (how: Removal) => {
    switch (how.kind) {
      case "branch":
        return t("models.route.removeBranch", "Remove this branch");
      case "step":
        return t("models.route.removeStep", "Remove step");
      case "keep":
        return how.edge === "then" ? t("models.route.keepThen", "Remove, keep Then") : t("models.route.keepElse", "Remove, keep Else");
      case "keepBranch":
        return t("models.route.keepBranch", "Remove, keep the {weight} branch", { weight: weightLabel(how.branch.weight) });
    }
  };
  const [only] = ways;
  if (ways.length === 1 && only)
    return (
      <Button variant="danger-secondary" size="sm" leadingIcon={icons.delete} onClick={() => ctx.onRemove(step.id, only)}>
        {label(only)}
      </Button>
    );
  return (
    <Menu>
      <Menu.Trigger>
        <Button variant="danger-secondary" size="sm" leadingIcon={icons.delete}>
          {t("models.route.remove", "Remove")}
        </Button>
      </Menu.Trigger>
      <Menu.Content align="end">
        {ways.map((how, i) => (
          <Menu.Item key={i} variant="danger" onClick={() => ctx.onRemove(step.id, how)}>
            {label(how)}
          </Menu.Item>
        ))}
      </Menu.Content>
    </Menu>
  );
}

export function ModelPanel({ ctx, step }: { ctx: InspectorContext; step: DraftModel }) {
  const set = (change: Partial<DraftModel>, coalesce?: string) => update(ctx, step, (s) => ({ ...s, ...change }), coalesce);
  return (
    <FlowWorkspace.Panel
      title={kindLabel.model()}
      description={t("models.route.modelHint", "Calls a model; when it fails as checked below, its fallback takes over.")}
      actions={<RemoveMenu ctx={ctx} step={step} />}
    >
      <StepProblems ctx={ctx} id={step.id} />
      <ModelPicker
        label={t("models.route.model", "Model")}
        appearance="default"
        size="md"
        fullWidth
        placeholder={t("models.chooseModel", "Choose a model")}
        value={step.model}
        onValueChange={(model: string) => set({ model })}
        options={ctx.options}
        providers={ctx.providers}
        disabled={ctx.readOnly}
        error={fieldError(ctx, step.id, "model")}
      />
      <CheckboxGroup
        aria-label={t("models.route.fallsBackWhen", "Falls back when")}
        value={step.fall_back_on}
        disabled={ctx.readOnly}
        onValueChange={(v) => set({ fall_back_on: FAILURES.filter((f) => v.includes(f)) })}
      >
        <Text variant="small" weight="medium">
          {t("models.route.fallsBackWhen", "Falls back when")}
        </Text>
        {FAILURES.map((f: RouteFailure) => (
          <Checkbox key={f} value={f} label={failureLabel[f]()} description={failureHint[f]()} disabled={ctx.readOnly} />
        ))}
      </CheckboxGroup>
      {!step.fallback && !ctx.readOnly && (
        <Button variant="secondary" leadingIcon={icons.add} onClick={() => ctx.onAddFallback(step.id)}>
          {t("models.route.addFallbackModel", "Add a fallback model")}
        </Button>
      )}
      <Input
        label={t("models.route.timeout", "Timeout in seconds")}
        description={t("models.route.timeoutHint", "How long to wait for the first byte before falling back; empty uses the model's.")}
        inputMode="numeric"
        optional
        disabled={ctx.readOnly}
        value={step.timeout_seconds === null ? "" : String(step.timeout_seconds)}
        placeholder={t("models.route.timeoutPlaceholder", "The model's timeout")}
        error={
          fieldError(ctx, step.id, "timeout_seconds") ??
          (step.timeout_seconds !== null && (step.timeout_seconds < 1 || step.timeout_seconds > MAX_TIMEOUT)
            ? t("models.route.timeoutRange", "Enter 1 to 3600 seconds.")
            : undefined)
        }
        onChange={(e) => {
          const text = e.target.value.replace(/[^\d]/g, "");
          set({ timeout_seconds: text === "" ? null : Number(text) }, `timeout:${step.id}`);
        }}
      />
    </FlowWorkspace.Panel>
  );
}

function RuleValue({ ctx, step, rule }: { ctx: InspectorContext; step: DraftCondition; rule: DraftRule }) {
  const set = (next: RouteRule, coalesce?: string) =>
    update(ctx, step, (s) => ({ ...s, rules: s.rules.map((r) => (r.id === rule.id ? { ...next, id: rule.id } : r)) }), coalesce);
  const keys = useQuery({ ...allKeysQuery(ctx.org, ctx.project), enabled: rule.field === "key" });
  const members = useQuery({ ...projectMembersQuery(ctx.org, ctx.project), enabled: rule.field === "member" });
  const accounts = useQuery({ ...serviceAccountsQuery(ctx.org, ctx.project), enabled: rule.field === "service_account" });
  const disabled = ctx.readOnly;
  const setOps = [
    { value: "in", label: t("models.route.op.in", "Is one of") },
    { value: "not_in", label: t("models.route.op.notIn", "Is none of") },
  ];
  switch (rule.field) {
    case "header": {
      const presence = rule.op === "present" || rule.op === "absent";
      return (
        <>
          <Input
            aria-label={t("models.route.headerName", "Header name")}
            placeholder={HEADER_EXAMPLE}
            value={rule.name}
            disabled={disabled}
            onChange={(e) => set({ ...rule, name: e.target.value.toLowerCase() }, `name:${rule.id}`)}
          />
          <Select
            aria-label={t("models.route.comparison", "Comparison")}
            fullWidth
            className={presence ? WIDE : undefined}
            disabled={disabled}
            value={rule.op}
            onValueChange={(op: string) => set({ ...rule, op: op as typeof rule.op, value: op === "present" || op === "absent" ? null : (rule.value ?? "") })}
            options={[
              { value: "equals", label: t("models.route.op.equals", "Is") },
              { value: "not_equals", label: t("models.route.op.notEquals", "Isn't") },
              { value: "contains", label: t("models.route.op.contains", "Contains") },
              { value: "present", label: t("models.route.op.present", "Is sent") },
              { value: "absent", label: t("models.route.op.absent", "Isn't sent") },
            ]}
          />
          {!presence && (
            <Input
              aria-label={t("models.route.headerValue", "Header value")}
              placeholder={t("models.route.headerValueExample", "paid")}
              maxLength={200}
              value={String(rule.value)}
              disabled={disabled}
              onChange={(e) => set({ ...rule, value: e.target.value }, `value:${rule.id}`)}
            />
          )}
        </>
      );
    }
    case "format":
      return (
        <>
          <Select
            aria-label={t("models.route.comparison", "Comparison")}
            fullWidth
            disabled={disabled}
            value={rule.op}
            onValueChange={(op: string) => set({ ...rule, op: op as typeof rule.op })}
            options={setOps}
          />
          <Select
            aria-label={t("models.route.formats", "API formats")}
            multiple
            fullWidth
            className={WIDE}
            disabled={disabled}
            placeholder={t("models.route.chooseFormats", "Choose formats")}
            value={rule.value}
            onValueChange={(v: string[]) => set({ ...rule, value: FORMATS.filter((f) => v.includes(f)) })}
            options={FORMATS.map((f) => ({ value: f, label: dialectLabel(f) }))}
          />
        </>
      );
    case "feature":
      return (
        <>
          <Select
            aria-label={t("models.route.comparison", "Comparison")}
            fullWidth
            disabled={disabled}
            value={rule.op}
            onValueChange={(op: string) => set({ ...rule, op: op as typeof rule.op })}
            options={[
              { value: "used", label: t("models.route.op.used", "Uses") },
              { value: "not_used", label: t("models.route.op.notUsed", "Doesn't use") },
            ]}
          />
          <Select
            aria-label={fieldLabel.feature()}
            fullWidth
            className={WIDE}
            disabled={disabled}
            value={rule.value}
            onValueChange={(v: string) => set({ ...rule, value: v as RouteFeature })}
            options={FEATURES.map((f) => ({ value: f, label: featureLabel[f]() }))}
          />
        </>
      );
    case "input_tokens":
      return (
        <>
          <Select
            aria-label={t("models.route.comparison", "Comparison")}
            fullWidth
            disabled={disabled}
            value={rule.op}
            onValueChange={(op: string) => set({ ...rule, op: op as typeof rule.op })}
            options={[
              { value: "at_least", label: t("models.route.op.atLeast", "At least") },
              { value: "below", label: t("models.route.op.below", "Fewer than") },
            ]}
          />
          <Input
            aria-label={fieldLabel.input_tokens()}
            className={WIDE}
            inputMode="numeric"
            disabled={disabled}
            value={rule.value ? String(rule.value) : ""}
            placeholder="100000"
            onChange={(e) => {
              const text = e.target.value.replace(/[^\d]/g, "");
              set({ ...rule, value: text === "" ? 0 : Number(text) }, `tokens:${rule.id}`);
            }}
          />
        </>
      );
    case "key":
    case "member":
    case "service_account": {
      const options =
        rule.field === "key"
          ? listed(keys.data, (k) => ({ value: k.id, label: k.name, description: k.redacted }))
          : rule.field === "member"
            ? listed(members.data, (m) => ({ value: m.user.id, label: m.user.name }))
            : listed(accounts.data, (a) => ({ value: a.id, label: a.name }));
      const loading = rule.field === "key" ? keys.isPending : rule.field === "member" ? members.isPending : accounts.isPending;
      return (
        <>
          <Select
            aria-label={t("models.route.comparison", "Comparison")}
            fullWidth
            disabled={disabled}
            value={rule.op}
            onValueChange={(op: string) => set({ ...rule, op: op as typeof rule.op })}
            options={setOps}
          />
          <Combobox
            aria-label={fieldLabel[rule.field]()}
            className={WIDE}
            multiple
            disabled={disabled}
            loading={loading}
            placeholder={t("models.route.chooseWho", "Search by name")}
            value={rule.value}
            onValueChange={(v: string[]) => set({ ...rule, value: v } as RouteRule)}
            options={options}
          />
        </>
      );
    }
  }
}

const WIDE = "col-span-2";

const listed = <T, O>(items: T[] | undefined, option: (item: T) => O): O[] => (items ?? []).map(option);

function blankRule(field: RouteRule["field"], id: string): DraftRule {
  switch (field) {
    case "header":
      return { id, field, name: "", op: "equals", value: "" };
    case "format":
      return { id, field, op: "in", value: ["anthropic_messages"] };
    case "feature":
      return { id, field, op: "used", value: "images" };
    case "input_tokens":
      return { id, field, op: "at_least", value: 100_000 };
    default:
      return { id, field, op: "in", value: [] };
  }
}

export function ConditionPanel({ ctx, step }: { ctx: InspectorContext; step: DraftCondition }) {
  const set = (change: (s: DraftCondition) => DraftCondition) => update(ctx, step, change);
  return (
    <FlowWorkspace.Panel
      title={kindLabel.condition()}
      description={t("models.route.conditionHint", "Requests that match the rules take Then; every other request takes Else.")}
      actions={<RemoveMenu ctx={ctx} step={step} />}
    >
      <StepProblems ctx={ctx} id={step.id} />
      {step.rules.length > 1 && (
        <Segmented
          aria-label={t("models.route.match", "How rules combine")}
          fullWidth
          disabled={ctx.readOnly}
          value={step.match}
          onValueChange={(v) => set((s) => ({ ...s, match: v === "any" ? "any" : "all" }))}
          options={[
            { value: "all", label: t("models.route.matchAll", "All rules hold") },
            { value: "any", label: t("models.route.matchAny", "Any rule holds") },
          ]}
        />
      )}
      {step.rules.map((rule, i) => (
        <section key={rule.id} aria-labelledby={`${rule.id}-label`} className="flex flex-col gap-8">
          <div className="flex items-center justify-between gap-8">
            <Text id={`${rule.id}-label`} variant="small" weight="medium">
              {t("models.route.ruleN", "Rule {n}", { n: formatNumber(i + 1) })}
            </Text>
            {!ctx.readOnly && step.rules.length > 1 && (
              <Button
                icon={icons.close}
                size="xs"
                variant="ghost"
                aria-label={t("models.route.removeRule", "Remove rule {n}", { n: formatNumber(i + 1) })}
                onClick={() => set((s) => ({ ...s, rules: s.rules.filter((r) => r.id !== rule.id) }))}
              />
            )}
          </div>
          <div className="grid grid-cols-2 gap-8">
            <Select
              aria-label={t("models.route.ruleField", "What the rule reads")}
              fullWidth
              disabled={ctx.readOnly}
              value={rule.field}
              onValueChange={(field: string) =>
                set((s) => ({ ...s, rules: s.rules.map((r) => (r.id === rule.id ? blankRule(field as RouteRule["field"], r.id) : r)) }))
              }
              options={(Object.keys(fieldLabel) as RouteRule["field"][]).map((f) => ({ value: f, label: fieldLabel[f]() }))}
            />
            <RuleValue ctx={ctx} step={step} rule={rule} />
          </div>
        </section>
      ))}
      {!ctx.readOnly && step.rules.length < MAX_RULES && (
        <Button variant="secondary" leadingIcon={icons.add} onClick={() => set((s) => ({ ...s, rules: [...s.rules, defaultRule()] }))}>
          {t("models.route.addRule", "Add rule")}
        </Button>
      )}
    </FlowWorkspace.Panel>
  );
}

export function SplitPanel({ ctx, step }: { ctx: InspectorContext; step: DraftSplit }) {
  const set = (change: (s: DraftSplit) => DraftSplit, coalesce?: string) => update(ctx, step, change, coalesce);
  const total = step.branches.reduce((n, b) => n + b.weight, 0);
  const headerIssue = step.header.trim() ? headerProblem(step.header) : undefined;
  return (
    <FlowWorkspace.Panel
      title={kindLabel.split()}
      description={t("models.route.splitHint", "Sends each request down one branch, by the branches' weights.")}
      actions={<RemoveMenu ctx={ctx} step={step} />}
    >
      <StepProblems ctx={ctx} id={step.id} />
      <Segmented
        aria-label={t("models.route.splitBy", "How requests are drawn")}
        fullWidth
        disabled={ctx.readOnly}
        value={step.by}
        onValueChange={(v) => set((s) => ({ ...s, by: v === "caller" || v === "header" ? v : "request" }))}
        options={[
          { value: "request", label: t("models.route.perRequest", "Per request") },
          { value: "caller", label: t("models.route.perCaller", "Per caller") },
          { value: "header", label: t("models.route.byHeader", "By header") },
        ]}
      />
      <Text variant="small" tone="secondary">
        {step.by === "caller"
          ? t("models.route.perCallerHint", "Each key, member or service account stays on one branch, for A/B tests.")
          : step.by === "header"
            ? t("models.route.byHeaderHint", "Each value of the header stays on one branch, such as one per end user.")
            : t("models.route.perRequestHint", "Every request is drawn anew.")}
      </Text>
      {step.by === "header" && (
        <Input
          label={t("models.route.splitHeader", "Header")}
          description={t("models.route.splitHeaderHint", "Requests without it are drawn anew.")}
          placeholder={SPLIT_HEADER_EXAMPLE}
          value={step.header}
          disabled={ctx.readOnly}
          error={fieldError(ctx, step.id, "header") ?? (headerIssue ? headerHint(headerIssue) : undefined)}
          onChange={(e) => set((s) => ({ ...s, header: e.target.value.toLowerCase() }), `header:${step.id}`)}
        />
      )}
      {step.branches.map((b, i) => (
        <div key={b.id} className="flex items-end gap-8">
          <Input
            label={t("models.route.branchN", "Branch {n}: {name}", { n: formatNumber(i + 1), name: ctx.stepName(b.step) })}
            inputMode="numeric"
            className="min-w-0 flex-1"
            disabled={ctx.readOnly}
            value={String(b.weight)}
            suffix="%"
            onChange={(e) => {
              const text = e.target.value.replace(/[^\d]/g, "").slice(0, 2);
              set((s) => ({ ...s, branches: s.branches.map((x) => (x.id === b.id ? { ...x, weight: text === "" ? 0 : Number(text) } : x)) }), `weight:${b.id}`);
            }}
          />
          {!ctx.readOnly && step.branches.length > 2 && (
            <Button
              size="sm"
              icon={icons.close}
              variant="ghost"
              aria-label={t("models.route.removeBranchN", "Remove branch {n}", { n: formatNumber(i + 1) })}
              onClick={() => ctx.onRemove(b.step.id, { kind: "branch" })}
            />
          )}
        </div>
      ))}
      <KeyValue
        items={[
          {
            label: t("models.route.total", "Total"),
            value: total === 100 ? t("models.route.totalOk", "100%") : t("models.route.totalOff", "{total}%, not 100%", { total: formatNumber(total) }),
          },
        ]}
      />
      {!ctx.readOnly && (
        <div className="flex flex-wrap gap-8">
          {step.branches.length < MAX_BRANCHES && (
            <Button
              variant="secondary"
              leadingIcon={icons.add}
              onClick={() => set((s) => ({ ...s, branches: shareEvenly([...s.branches, { id: freshId("b"), weight: 0, step: emptyModel() }]) }))}
            >
              {t("models.route.addBranch", "Add branch")}
            </Button>
          )}
          {total !== 100 && (
            <Button variant="secondary" onClick={() => set((s) => ({ ...s, branches: shareEvenly(s.branches) }))}>
              {t("models.route.evenOut", "Share evenly")}
            </Button>
          )}
        </div>
      )}
    </FlowWorkspace.Panel>
  );
}

export function AliasPanel({
  name,
  onName,
  nameError,
  readOnly,
  shadows,
  problems,
  templates,
}: {
  name: string;
  onName: (name: string) => void;
  nameError: string | undefined;
  readOnly: boolean;
  shadows: boolean;
  problems: number;
  templates?: { value: Template | null; onPick: (template: Template) => void } | undefined;
}) {
  const shown = name.trim() || t("models.alias.nameExample", "fast");
  const base = useMemo(() => `${window.location.origin}/v1`, []);
  const tabs = [
    {
      label: SNIPPETS.curl,
      lang: "bash" as const,
      code: `curl ${base}/chat/completions \\\n  -H "Authorization: Bearer $KAESO_API_KEY" \\\n  -H "Content-Type: application/json" \\\n  -d '{"model": "${shown}", "messages": [{"role": "user", "content": "Hello"}]}'`,
    },
    {
      label: SNIPPETS.python,
      lang: "python" as const,
      code: `from openai import OpenAI\n\nclient = OpenAI(base_url="${base}")\nreply = client.chat.completions.create(\n    model="${shown}",\n    messages=[{"role": "user", "content": "Hello"}],\n)`,
    },
    {
      label: SNIPPETS.ts,
      lang: "ts" as const,
      code: `import OpenAI from "openai";\n\nconst client = new OpenAI({ baseURL: "${base}" });\nconst reply = await client.chat.completions.create({\n  model: "${shown}",\n  messages: [{ role: "user", content: "Hello" }],\n});`,
    },
  ];
  return (
    <FlowWorkspace.Panel
      title={t("models.route.alias", "Alias")}
      description={t("models.route.aliasHint", "Apps send its name as model, and the route picks the model that answers.")}
    >
      <Input
        label={t("common.name", "Name")}
        value={name}
        disabled={readOnly}
        maxLength={200}
        placeholder={t("models.alias.nameExample", "fast")}
        error={nameError}
        onChange={(e) => onName(e.target.value)}
      />
      {shadows && name.trim() && (
        <Banner tone="warning">
          {t("models.alias.shadows", "Apps that send `{name}` get this alias, not the model with the same name.", { name: name.trim() })}
        </Banner>
      )}
      {templates && (
        <Select
          label={t("models.route.startFrom", "Start from")}
          fullWidth
          value={templates.value}
          placeholder={t("models.route.ownRoute", "Your own route")}
          onValueChange={(v: string) => templates.onPick(v as Template)}
          options={TEMPLATES.map((x) => ({ value: x, label: templateLabel[x](), description: templateHint[x]() }))}
        />
      )}
      {problems > 0 && (
        <Banner tone="warning">
          {t("models.route.problemCount", "{count, plural, one {# step needs} other {# steps need}} a fix before you can save.", { count: problems })}
        </Banner>
      )}
      <div className="flex flex-col gap-8">
        <Text variant="small" weight="medium">
          {t("models.route.callIt", "Call it")}
        </Text>
        <Text variant="small" tone="secondary">
          {tRich("models.route.callItHint", "Any app format works; send the alias as {field}.", { field: <Code>{MODEL_FIELD}</Code> })}
        </Text>
        <CodeBlock tabs={tabs} collapseAfter={0} wrap="phone" />
      </div>
    </FlowWorkspace.Panel>
  );
}
