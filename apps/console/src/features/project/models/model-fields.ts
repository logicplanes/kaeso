import type { Capability } from "@logicplanes/kaeso-ui";
import { formatContext, formatUSD } from "@logicplanes/kaeso-ui/format";
import type { CatalogueModelView, Fields } from "~/api/generated/types";
import { t } from "~/i18n";

export function capabilities(f: Fields): Capability[] {
  const out: Capability[] = [];
  if (f.tool_call) out.push("tools");
  if (f.reasoning) out.push("reasoning");
  if (f.structured_output) out.push("structuredOutput");
  if (f.input_modalities?.includes("image")) out.push("vision");
  if (f.input_modalities?.includes("pdf")) out.push("pdf");
  return out;
}

export function capabilityOptions(): { value: Capability; label: string }[] {
  return [
    { value: "tools", label: t("models.cap.tools", "Tool calling") },
    { value: "reasoning", label: t("models.cap.reasoning", "Reasoning") },
    { value: "structuredOutput", label: t("models.cap.structuredOutput", "Structured output") },
    { value: "vision", label: t("models.cap.vision", "Image input") },
    { value: "pdf", label: t("models.cap.pdf", "PDF input") },
  ];
}

export function contextLabel(tokens: number | null | undefined): string {
  return tokens ? formatContext(tokens) : "—";
}

export function priceLabel(f: Fields): string {
  if (!f.input_price && !f.output_price) return "—";
  const short = (v: string | null | undefined) => (v ? formatUSD(v, Number(v) % 1 === 0 ? { decimals: 0 } : {}) : "—");
  if (f.input_price && f.output_price && Number(f.output_price) === 0) return short(f.input_price);
  return `${short(f.input_price)} / ${short(f.output_price)}`;
}

export function overriddenFields(m: CatalogueModelView): string[] {
  return Object.entries(m.overrides)
    .filter(([, v]) => v !== null && v !== undefined)
    .map(([k]) => k);
}

export function dialectPill(d: string): string {
  return { openai_responses: "responses", openai_chat: "chat", anthropic_messages: "messages", openai_embeddings: "embeddings" }[d] ?? d;
}

export type FieldKind = "text" | "number" | "price" | "bool" | "list" | "dialect" | "date";

export function fieldRows(): { key: keyof Fields; label: string; kind: FieldKind }[] {
  return [
    { key: "display_name", label: t("models.field.displayName", "Display name"), kind: "text" },
    { key: "input_modalities", label: t("models.field.inputs", "Inputs"), kind: "list" },
    { key: "output_modalities", label: t("models.field.outputs", "Outputs"), kind: "list" },
    { key: "context_limit", label: t("models.field.context", "Context"), kind: "number" },
    { key: "output_limit", label: t("models.field.outputLimit", "Output limit"), kind: "number" },
    { key: "input_price", label: t("models.field.inputPrice", "Input price"), kind: "price" },
    { key: "output_price", label: t("models.field.outputPrice", "Output price"), kind: "price" },
    { key: "cache_read_price", label: t("models.field.cacheReadPrice", "Cache read price"), kind: "price" },
    { key: "cache_write_price", label: t("models.field.cacheWritePrice", "Cache write price"), kind: "price" },
    { key: "egress_dialect", label: t("models.field.sendsAs", "Sends as"), kind: "dialect" },
    { key: "tool_call", label: t("models.cap.tools", "Tool calling"), kind: "bool" },
    { key: "reasoning", label: t("models.cap.reasoning", "Reasoning"), kind: "bool" },
    { key: "structured_output", label: t("models.cap.structuredOutput", "Structured output"), kind: "bool" },
    { key: "temperature", label: t("models.field.temperature", "Temperature"), kind: "bool" },
    { key: "adaptive_thinking", label: t("models.field.adaptiveThinking", "Adaptive thinking"), kind: "bool" },
    { key: "forced_tool_choice", label: t("models.field.forcedToolChoice", "Forced tool choice"), kind: "bool" },
    { key: "efforts", label: t("models.field.efforts", "Efforts"), kind: "list" },
    { key: "timeout_seconds", label: t("models.field.timeout", "Timeout"), kind: "number" },
    { key: "release_date", label: t("models.field.released", "Released"), kind: "date" },
  ];
}
