import type { Capability } from "@logicplanes/kaeso-ui";
import { formatContext, formatPrice, formatUSD } from "@logicplanes/kaeso-ui/format";
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
  const short = (v: string | null | undefined) => (v ? (Number(v) % 1 === 0 ? formatUSD(v, { decimals: 0 }) : formatPrice(v)) : "—");
  if (f.input_price && f.output_price && Number(f.output_price) === 0) return short(f.input_price);
  return `${short(f.input_price)} / ${short(f.output_price)}`;
}

export function tierLabels(f: Fields): string[] {
  const price = (v: string | null | undefined) => (v ? formatPrice(v) : "—");
  return (f.price_tiers ?? []).map((tier) =>
    t("models.priceTier", "Above {context}: {input} / {output}", {
      context: formatContext(tier.context_above),
      input: price(tier.input_price ?? f.input_price),
      output: price(tier.output_price ?? f.output_price),
    }),
  );
}

export function unitPriceLabels(f: Fields): string[] {
  return (f.unit_prices ?? []).map((u) => {
    const unit = u.per === "image" ? t("models.unit.image", "image") : u.per === "megapixel" ? t("models.unit.megapixel", "megapixel") : u.per;
    const side = u.of === "input" ? t("models.unit.input", "input") : t("models.unit.output", "output");
    const conditions = [u.size, u.quality, u.resolution, u.audio].filter((c): c is string => !!c);
    const label = t("models.unitPrice", "{price} per {unit} of {side}", { price: formatUSD(u.usd), unit, side });
    const shown = conditions.length ? `${label} (${conditions.join(t("models.listSeparator", ", "))})` : label;
    return u.estimate ? t("models.unitPriceEstimate", "{price}, estimated", { price: shown }) : shown;
  });
}

export function overriddenFields(m: CatalogueModelView): string[] {
  return Object.entries(m.overrides)
    .filter(([, v]) => v !== null && v !== undefined)
    .map(([k]) => k);
}

export function dialectPill(d: string): string {
  return (
    {
      openai_responses: "responses",
      openai_chat: "chat",
      anthropic_messages: "messages",
      openai_embeddings: "embeddings",
      gemini: "gemini",
      gemini_embeddings: "embeddings",
      openai_images: "images",
      minimax_images: "images",
    }[d] ?? d
  );
}

export type FieldKind = "text" | "number" | "price" | "tiers" | "units" | "bool" | "list" | "dialect" | "date";

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
    { key: "cache_write_1h_price", label: t("models.field.cacheWrite1hPrice", "1-hour cache write price"), kind: "price" },
    { key: "price_tiers", label: t("models.field.priceTiers", "Long-context prices"), kind: "tiers" },
    { key: "input_image_price", label: t("models.field.inputImagePrice", "Image input price"), kind: "price" },
    { key: "output_image_price", label: t("models.field.outputImagePrice", "Image output price"), kind: "price" },
    { key: "cache_read_image_price", label: t("models.field.cacheReadImagePrice", "Image cache read price"), kind: "price" },
    { key: "unit_prices", label: t("models.field.unitPrices", "Prices per unit"), kind: "units" },
    { key: "egress_dialect", label: t("models.field.sendsAs", "Sends as"), kind: "dialect" },
    { key: "tool_call", label: t("models.cap.tools", "Tool calling"), kind: "bool" },
    { key: "reasoning", label: t("models.cap.reasoning", "Reasoning"), kind: "bool" },
    { key: "structured_output", label: t("models.cap.structuredOutput", "Structured output"), kind: "bool" },
    { key: "temperature", label: t("models.field.temperature", "Temperature"), kind: "bool" },
    { key: "adaptive_thinking", label: t("models.field.adaptiveThinking", "Adaptive thinking"), kind: "bool" },
    { key: "thinking_binding", label: t("models.field.thinkingBinding", "Thinking binding"), kind: "bool" },
    { key: "forced_tool_choice", label: t("models.field.forcedToolChoice", "Forced tool choice"), kind: "bool" },
    { key: "efforts", label: t("models.field.efforts", "Efforts"), kind: "list" },
    { key: "timeout_seconds", label: t("models.field.timeout", "Timeout"), kind: "number" },
    { key: "release_date", label: t("models.field.released", "Released"), kind: "date" },
  ];
}
