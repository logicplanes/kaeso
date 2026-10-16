import { Checkbox, CheckboxGroup, Field, Input, NumberInput, Select, Switch, Text } from "@logicplanes/kaeso-ui";
import type { Fields } from "~/api/generated/types";
import { DIALECT_IDS, dialectLabel } from "~/format/labels";
import { t } from "~/i18n";

export interface FieldDraft {
  display_name: string;
  input_modalities: string[];
  output_modalities: string[];
  context_limit: string;
  output_limit: string;
  input_price: string;
  output_price: string;
  cache_read_price: string;
  cache_write_price: string;
  egress_dialect: string;
  tool_call: boolean;
  reasoning: boolean;
  structured_output: boolean;
  temperature: boolean;
  adaptive_thinking: boolean;
  efforts: string[];
  timeout_seconds: string;
}

const PROVIDER_DIALECTS: Record<string, string[]> = {
  openai: ["openai_responses", "openai_chat", "openai_embeddings"],
  anthropic: ["anthropic_messages"],
};

const INPUTS = ["text", "image", "pdf", "audio", "video"];
const OUTPUTS = ["text", "image", "audio", "embedding"];
const EFFORTS = ["none", "minimal", "low", "medium", "high", "xhigh", "max"];

const num = (v: number | null | undefined) => (v === null || v === undefined ? "" : String(v));

export function draftFrom(f: Fields): FieldDraft {
  return {
    display_name: f.display_name ?? "",
    input_modalities: f.input_modalities ?? [],
    output_modalities: f.output_modalities ?? [],
    context_limit: num(f.context_limit),
    output_limit: num(f.output_limit),
    input_price: f.input_price ?? "",
    output_price: f.output_price ?? "",
    cache_read_price: f.cache_read_price ?? "",
    cache_write_price: f.cache_write_price ?? "",
    egress_dialect: f.egress_dialect ?? "",
    tool_call: !!f.tool_call,
    reasoning: !!f.reasoning,
    structured_output: !!f.structured_output,
    temperature: !!f.temperature,
    adaptive_thinking: !!f.adaptive_thinking,
    efforts: f.efforts ?? [],
    timeout_seconds: num(f.timeout_seconds),
  };
}

function fieldsOf(d: FieldDraft, provider: string): Fields {
  const int = (v: string) => (v.trim() === "" ? null : Number(v));
  const text = (v: string) => (v.trim() === "" ? null : v.trim());
  return {
    display_name: text(d.display_name),
    input_modalities: d.input_modalities,
    output_modalities: d.output_modalities,
    context_limit: int(d.context_limit),
    output_limit: int(d.output_limit),
    input_price: text(d.input_price),
    output_price: text(d.output_price),
    cache_read_price: text(d.cache_read_price),
    cache_write_price: text(d.cache_write_price),
    egress_dialect: text(d.egress_dialect),
    tool_call: d.tool_call,
    reasoning: d.reasoning,
    structured_output: d.structured_output,
    temperature: d.temperature,
    adaptive_thinking: provider === "anthropic" ? d.adaptive_thinking : null,
    efforts: d.efforts,
    timeout_seconds: int(d.timeout_seconds),
  };
}

const same = (a: unknown, b: unknown) => JSON.stringify(a ?? null) === JSON.stringify(b ?? null);
const samePrice = (a: string | null | undefined, b: string | null | undefined) => (a && b ? Number(a) === Number(b) : same(a || null, b || null));
const PRICES = new Set(["input_price", "output_price", "cache_read_price", "cache_write_price"]);
const asBool = (v: unknown) => !!v;
const BOOLS = new Set(["tool_call", "reasoning", "structured_output", "temperature", "adaptive_thinking"]);
const asList = (v: unknown) => (Array.isArray(v) ? [...(v as string[])].sort() : []);
const LISTS = new Set(["input_modalities", "output_modalities", "efforts"]);

export function changedFields(d: FieldDraft, base: Fields, provider: string): Partial<Fields> {
  const next = fieldsOf(d, provider);
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(next)) {
    const b = base[k as keyof Fields];
    const equal = PRICES.has(k)
      ? samePrice(v as string | null, b as string | null)
      : BOOLS.has(k)
        ? asBool(v) === asBool(b)
        : LISTS.has(k)
          ? same(asList(v), asList(b))
          : same(v, b);
    if (!equal) out[k] = v;
  }
  return out as Partial<Fields>;
}

export function allFields(d: FieldDraft, provider: string): Partial<Fields> {
  return Object.fromEntries(Object.entries(fieldsOf(d, provider)).filter(([, v]) => v !== null && !(Array.isArray(v) && v.length === 0))) as Partial<Fields>;
}

export function draftProblem(d: FieldDraft, custom: boolean): string | null {
  const price = (v: string) => v.trim() === "" || (/^\d+(\.\d+)?$/.test(v.trim()) && Number.isFinite(Number(v)));
  const whole = (v: string) => v.trim() === "" || /^\d+$/.test(v.trim());
  if (![d.input_price, d.output_price, d.cache_read_price, d.cache_write_price].every(price))
    return t("models.problem.price", "Prices are decimal amounts in USD, like 3.00.");
  if (![d.context_limit, d.output_limit, d.timeout_seconds].every(whole)) return t("models.problem.whole", "Limits and the timeout are whole numbers.");
  if (custom) {
    if (!d.input_modalities.length || !d.output_modalities.length) return t("models.problem.modalities", "A custom model needs its inputs and outputs.");
    if (!d.context_limit || !d.output_limit) return t("models.problem.limits", "A custom model needs its context and output limits.");
    if (!d.input_price || !d.output_price) return t("models.problem.prices", "A custom model needs its input and output prices.");
    if (!d.egress_dialect) return t("models.problem.format", "A custom model needs the format Kaeso sends it.");
  }
  return null;
}

export function ModelForm({ value, onChange, provider }: { value: FieldDraft; onChange: (d: FieldDraft) => void; provider: string }) {
  const set = (patch: Partial<FieldDraft>) => onChange({ ...value, ...patch });
  const price = (key: "input_price" | "output_price" | "cache_read_price" | "cache_write_price", label: string) => (
    <NumberInput
      label={label}
      value={value[key]}
      onValueChange={(v) => set({ [key]: v })}
      min="0"
      step="0.01"
      prefix="$"
      unit={t("models.unit.perMillionTokens", "per M tokens")}
    />
  );
  return (
    <div className="flex flex-col gap-20">
      <Input
        label={t("models.field.displayName", "Display name")}
        value={value.display_name}
        onChange={(e) => set({ display_name: e.target.value })}
        maxLength={200}
      />
      <div className="grid grid-cols-2 gap-16">
        <Field label={t("models.field.inputs", "Inputs")}>
          <CheckboxGroup aria-label={t("models.field.inputs", "Inputs")} value={value.input_modalities} onValueChange={(v) => set({ input_modalities: v })}>
            {INPUTS.map((m) => (
              <Checkbox key={m} value={m} label={modalityLabel(m)} />
            ))}
          </CheckboxGroup>
        </Field>
        <Field label={t("models.field.outputs", "Outputs")}>
          <CheckboxGroup aria-label={t("models.field.outputs", "Outputs")} value={value.output_modalities} onValueChange={(v) => set({ output_modalities: v })}>
            {OUTPUTS.map((m) => (
              <Checkbox key={m} value={m} label={modalityLabel(m)} />
            ))}
          </CheckboxGroup>
        </Field>
      </div>
      <div className="grid grid-cols-1 gap-16 sm:grid-cols-2">
        <NumberInput
          label={t("models.field.context", "Context")}
          value={value.context_limit}
          onValueChange={(v) => set({ context_limit: v })}
          min="0"
          step="1"
          unit={t("models.unit.tokens", "tokens")}
        />
        <NumberInput
          label={t("models.field.outputLimit", "Output limit")}
          value={value.output_limit}
          onValueChange={(v) => set({ output_limit: v })}
          min="0"
          step="1"
          unit={t("models.unit.tokens", "tokens")}
        />
      </div>
      <div className="grid grid-cols-1 gap-16 sm:grid-cols-2">
        {price("input_price", t("models.field.inputPrice", "Input price"))}
        {price("output_price", t("models.field.outputPrice", "Output price"))}
        {price("cache_read_price", t("models.field.cacheReadPrice", "Cache read price"))}
        {price("cache_write_price", t("models.field.cacheWritePrice", "Cache write price"))}
      </div>
      <Select
        label={t("models.field.sendsAs", "Sends as")}
        description={t("models.sendsAsHint", "The format Kaeso sends to the provider.")}
        fullWidth
        placeholder={t("models.chooseFormat", "Choose a format")}
        value={value.egress_dialect || null}
        onValueChange={(v) => set({ egress_dialect: v })}
        options={(PROVIDER_DIALECTS[provider] ?? DIALECT_IDS).map((d) => ({ value: d, label: dialectLabel(d) }))}
      />
      <div className="flex flex-col gap-12">
        <Switch label={t("models.cap.tools", "Tool calling")} checked={value.tool_call} onCheckedChange={(tool_call) => set({ tool_call })} />
        <Switch label={t("models.cap.reasoning", "Reasoning")} checked={value.reasoning} onCheckedChange={(reasoning) => set({ reasoning })} />
        <Switch
          label={t("models.cap.structuredOutput", "Structured output")}
          checked={value.structured_output}
          onCheckedChange={(structured_output) => set({ structured_output })}
        />
        <Switch
          label={t("models.acceptsTemperature", "Accepts temperature")}
          checked={value.temperature}
          onCheckedChange={(temperature) => set({ temperature })}
        />
        {provider === "anthropic" && (
          <Switch
            label={t("models.field.adaptiveThinking", "Adaptive thinking")}
            checked={value.adaptive_thinking}
            onCheckedChange={(adaptive_thinking) => set({ adaptive_thinking })}
          />
        )}
      </div>
      <Select
        label={t("models.effortLevels", "Effort levels")}
        description={t("models.effortLevelsHint", "The levels requests may send; none means the model takes no effort setting.")}
        multiple
        fullWidth
        placeholder={t("models.noEffortLevels", "No effort levels")}
        value={value.efforts}
        onValueChange={(efforts) => set({ efforts })}
        options={EFFORTS.map((e) => ({ value: e, label: e }))}
      />
      <NumberInput
        label={t("models.field.timeout", "Timeout")}
        description={t("models.timeoutHint", "Empty uses the instance's request timeout.")}
        value={value.timeout_seconds}
        onValueChange={(v) => set({ timeout_seconds: v })}
        min="1"
        step="1"
        unit={t("models.unit.seconds", "seconds")}
      />
      <Text variant="small" tone="secondary">
        {t("models.overrideNote", "Saved values override models.dev for this project only.")}
      </Text>
    </div>
  );
}

export function modalityLabel(m: string): string {
  switch (m) {
    case "text":
      return t("models.modality.text", "Text");
    case "image":
      return t("models.modality.image", "Images");
    case "pdf":
      return "PDF";
    case "audio":
      return t("models.modality.audio", "Audio");
    case "video":
      return t("models.modality.video", "Video");
    case "embedding":
      return t("models.modality.embedding", "Embeddings");
    default:
      return m;
  }
}
