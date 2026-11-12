import { forwardRef, type ReactElement } from "react";
import { cn } from "../../utils/cn.js";
import { icons } from "../../icons/vocabulary.js";
import { Button } from "../button/button.js";
import { Input } from "../input/input.js";
import { NumberInput } from "../input/number-input.js";
import { Textarea } from "../input/textarea.js";
import { Popover } from "../popover/popover.js";
import { Segmented } from "../segmented/segmented.js";
import { Select } from "../select/select.js";
import { Slider } from "../slider/slider.js";
import { useUiText, type UiText } from "../../i18n/context.js";
import { useFormatters } from "../../i18n/formatters.js";

export const KAESO_PARAMETER_PANEL_VARIANTS = {
  layout: {
    popover: { classes: "w-full gap-20", description: "Inside a 320px popover, as with a `trigger` (default)" },
    panel: { classes: "w-full gap-24", description: "A side panel or sheet: a little more room between groups" },
  },
} as const;
export const KAESO_PARAMETER_PANEL_DEFAULT_VARIANTS = { layout: "popover" } as const;

export type ReasoningEffort = "none" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max";
export type ReasoningSummary = "auto" | "concise" | "detailed" | "off";

export interface PlaygroundParameters {
  instructions?: string | undefined;
  temperature?: number | undefined;
  topP?: number | undefined;
  maxOutputTokens?: number | undefined;
  reasoningEffort?: ReasoningEffort | undefined;
  reasoningSummary?: ReasoningSummary | undefined;
  imageSize?: string | undefined;
  imageCount?: number | undefined;
  imageQuality?: string | undefined;
  voice?: string | undefined;
  durationSeconds?: number | undefined;
  translate?: boolean | undefined;
}

/** What an image model takes: how many images at once, its qualities, and whether it takes a size. */
export interface ImageParameterOptions {
  maxCount: number;
  qualities: readonly string[];
  sizes: boolean;
}

const IMAGE_SIZES = ["1024x1024", "1536x1024", "1024x1536"] as const;

function qualityLabel(t: UiText, quality: string): string {
  switch (quality) {
    case "auto":
      return t("parameterPanel.qualityAuto", "Automatic");
    case "low":
      return t("parameterPanel.qualityLow", "Low");
    case "medium":
      return t("parameterPanel.qualityMedium", "Medium");
    case "high":
      return t("parameterPanel.qualityHigh", "High");
    case "standard":
      return t("parameterPanel.qualityStandard", "Standard");
    case "hd":
      return t("parameterPanel.qualityHd", "HD");
    default:
      return quality;
  }
}
const MAX_IMAGES_SHOWN = 4;

function efforts(t: UiText): { value: ReasoningEffort; label: string }[] {
  return [
    { value: "none", label: t("parameterPanel.effortNone", "None") },
    { value: "minimal", label: t("parameterPanel.effortMinimal", "Minimal") },
    { value: "low", label: t("parameterPanel.effortLow", "Low") },
    { value: "medium", label: t("parameterPanel.effortMedium", "Medium") },
    { value: "high", label: t("parameterPanel.effortHigh", "High") },
    { value: "xhigh", label: t("parameterPanel.effortExtraHigh", "Extra high") },
    { value: "max", label: t("parameterPanel.effortMax", "Max") },
  ];
}

export interface ParameterPanelProps {
  value: PlaygroundParameters;
  onValueChange: (next: PlaygroundParameters) => void;
  defaults?: Pick<PlaygroundParameters, "temperature" | "topP"> | undefined;
  instructions?: boolean | undefined;
  sampling?: boolean | undefined;
  reasoning?: ReasoningEffort[] | boolean | undefined;
  maxOutputTokensLimit?: number | undefined;
  text?: boolean | undefined;
  image?: ImageParameterOptions | undefined;
  voices?: readonly string[] | boolean | undefined;
  duration?: boolean | undefined;
  translate?: boolean | undefined;
  disabled?: boolean | undefined;
  layout?: keyof typeof KAESO_PARAMETER_PANEL_VARIANTS.layout | undefined;
  className?: string | undefined;
  trigger?: ReactElement | undefined;
  title?: string | undefined;
  open?: boolean | undefined;
  defaultOpen?: boolean | undefined;
  onOpenChange?: ((open: boolean) => void) | undefined;
  side?: "top" | "bottom" | "left" | "right" | undefined;
  align?: "start" | "center" | "end" | undefined;
}

type PanelBodyProps = Omit<ParameterPanelProps, "trigger" | "title" | "open" | "defaultOpen" | "onOpenChange" | "side" | "align">;

function SliderRow({
  label,
  value,
  unset,
  min,
  max,
  step,
  digits,
  disabled,
  onChange,
}: {
  label: string;
  value: number;
  unset: boolean;
  min: number;
  max: number;
  step: number;
  digits: number;
  disabled: boolean;
  onChange: (v: number) => void;
}) {
  const t = useUiText();
  const { formatNumber } = useFormatters();
  const shown = formatNumber(value, { minimumFractionDigits: digits, maximumFractionDigits: digits });
  return (
    <div className="flex flex-col gap-8">
      <div className="flex items-baseline justify-between gap-12">
        <span className={cn("text-body font-medium", disabled ? "text-fg-disabled" : "text-fg")}>{label}</span>
        <span className={cn("text-small tabular", unset ? "text-fg-tertiary" : "text-fg-secondary")}>
          {unset ? t("parameterPanel.valueDefault", "{value} (default)", { value: shown }) : shown}
        </span>
      </div>
      <div className="px-8">
        <Slider
          aria-label={label}
          min={min}
          max={max}
          step={step}
          value={value}
          disabled={disabled}
          formatValue={(v) => formatNumber(v, { minimumFractionDigits: digits, maximumFractionDigits: digits })}
          onValueChange={(v) => onChange(Array.isArray(v) ? (v[0] ?? min) : v)}
        />
      </div>
    </div>
  );
}

const PanelBody = forwardRef<HTMLDivElement, PanelBodyProps>(function PanelBody(
  {
    value,
    onValueChange,
    defaults,
    instructions = true,
    sampling = true,
    reasoning = false,
    maxOutputTokensLimit,
    text = true,
    image,
    voices = false,
    duration = false,
    translate = false,
    disabled = false,
    layout = "popover",
    className,
  },
  ref,
) {
  const t = useUiText();
  const { formatNumber } = useFormatters();
  const set = (patch: Partial<PlaygroundParameters>) => onValueChange({ ...value, ...patch });
  const all = efforts(t);
  const shownEfforts = reasoning === true ? all : Array.isArray(reasoning) ? all.filter((e) => reasoning.includes(e.value)) : [];
  const changed =
    value.temperature !== undefined ||
    value.topP !== undefined ||
    value.maxOutputTokens !== undefined ||
    value.reasoningEffort !== undefined ||
    value.reasoningSummary !== undefined ||
    value.imageSize !== undefined ||
    value.imageCount !== undefined ||
    value.imageQuality !== undefined ||
    value.voice !== undefined ||
    value.durationSeconds !== undefined ||
    value.translate !== undefined;
  const modelDefault = t("parameterPanel.modelDefault", "Model default");
  const sizeLabel = (size: (typeof IMAGE_SIZES)[number]) => {
    const [w, h] = size.split("x");
    const shape =
      w === h
        ? t("parameterPanel.sizeSquare", "Square")
        : Number(w) > Number(h)
          ? t("parameterPanel.sizeLandscape", "Landscape")
          : t("parameterPanel.sizePortrait", "Portrait");
    return t("parameterPanel.sizeValue", "{shape}, {width} × {height}", { shape, width: w ?? "", height: h ?? "" });
  };
  const counts = image ? Array.from({ length: Math.min(MAX_IMAGES_SHOWN, Math.max(1, image.maxCount)) }, (_, i) => i + 1) : [];
  return (
    <div ref={ref} className={cn("flex min-w-0 flex-col", KAESO_PARAMETER_PANEL_VARIANTS.layout[layout].classes, className)}>
      {instructions && (
        <Textarea
          label={t("parameterPanel.systemInstructions", "System instructions")}
          rows={3}
          maxRows={8}
          placeholder={t("parameterPanel.instructionsPlaceholder", "You are a helpful assistant.")}
          value={value.instructions ?? ""}
          disabled={disabled}
          onChange={(e) => set({ instructions: e.target.value === "" ? undefined : e.target.value })}
        />
      )}
      {sampling && (
        <>
          <SliderRow
            label={t("parameterPanel.temperature", "Temperature")}
            value={value.temperature ?? defaults?.temperature ?? 1}
            unset={value.temperature === undefined}
            min={0}
            max={2}
            step={0.05}
            digits={2}
            disabled={disabled}
            onChange={(v) => set({ temperature: v })}
          />
          <SliderRow
            label={t("parameterPanel.topP", "Top P")}
            value={value.topP ?? defaults?.topP ?? 1}
            unset={value.topP === undefined}
            min={0}
            max={1}
            step={0.01}
            digits={2}
            disabled={disabled}
            onChange={(v) => set({ topP: v })}
          />
        </>
      )}
      {text && (
        <NumberInput
          label={t("parameterPanel.maxOutputTokens", "Max output tokens")}
          size="sm"
          placeholder={t("parameterPanel.modelDefault", "Model default")}
          align="start"
          min="1"
          {...(maxOutputTokensLimit
            ? {
                max: String(maxOutputTokensLimit),
                description: t("parameterPanel.maxOutputTokensLimit", "Up to {max} for this model", { max: formatNumber(maxOutputTokensLimit) }),
              }
            : {})}
          step="256"
          value={value.maxOutputTokens === undefined ? "" : String(value.maxOutputTokens)}
          disabled={disabled}
          onValueChange={(v) => {
            const n = Number(v);
            set({ maxOutputTokens: v.trim() === "" || !Number.isFinite(n) || n <= 0 ? undefined : Math.round(n) });
          }}
        />
      )}
      {shownEfforts.length > 0 && (
        <>
          <Select
            label={t("parameterPanel.reasoningEffort", "Reasoning effort")}
            size="sm"
            fullWidth
            disabled={disabled}
            value={value.reasoningEffort ?? "default"}
            options={[{ value: "default", label: t("parameterPanel.modelDefault", "Model default") }, ...shownEfforts]}
            onValueChange={(v: string) => set({ reasoningEffort: v === "default" ? undefined : (v as ReasoningEffort) })}
          />
          <div className="flex flex-col gap-8">
            <span className={cn("text-body font-medium", disabled ? "text-fg-disabled" : "text-fg")}>
              {t("parameterPanel.reasoningSummary", "Reasoning summary")}
            </span>
            <Segmented
              aria-label={t("parameterPanel.reasoningSummary", "Reasoning summary")}
              size="sm"
              fullWidth
              disabled={disabled}
              value={value.reasoningSummary ?? "auto"}
              options={[
                { value: "auto", label: t("parameterPanel.summaryAuto", "Auto") },
                { value: "concise", label: t("parameterPanel.summaryConcise", "Concise") },
                { value: "detailed", label: t("parameterPanel.summaryDetailed", "Detailed") },
                { value: "off", label: t("parameterPanel.summaryOff", "Off") },
              ]}
              onValueChange={(v) => set({ reasoningSummary: v === "auto" ? undefined : (v as ReasoningSummary) })}
            />
          </div>
        </>
      )}
      {image?.sizes && (
        <Select
          label={t("parameterPanel.imageSize", "Image size")}
          size="sm"
          fullWidth
          disabled={disabled}
          value={value.imageSize ?? "default"}
          options={[{ value: "default", label: modelDefault }, ...IMAGE_SIZES.map((v) => ({ value: v, label: sizeLabel(v) }))]}
          onValueChange={(v: string) => set({ imageSize: v === "default" ? undefined : v })}
        />
      )}
      {counts.length > 1 && (
        <Select
          label={t("parameterPanel.imageCount", "Images per prompt")}
          size="sm"
          fullWidth
          disabled={disabled}
          value={String(value.imageCount ?? 1)}
          options={counts.map((n) => ({ value: String(n), label: formatNumber(n) }))}
          onValueChange={(v: string) => set({ imageCount: v === "1" ? undefined : Number(v) })}
        />
      )}
      {image && image.qualities.length > 0 && (
        <Select
          label={t("parameterPanel.imageQuality", "Image quality")}
          size="sm"
          fullWidth
          disabled={disabled}
          value={value.imageQuality ?? "default"}
          options={[{ value: "default", label: modelDefault }, ...image.qualities.map((q) => ({ value: q, label: qualityLabel(t, q) }))]}
          onValueChange={(v: string) => set({ imageQuality: v === "default" ? undefined : v })}
        />
      )}
      {Array.isArray(voices) && voices.length > 0 ? (
        <Select
          label={t("parameterPanel.voice", "Voice")}
          size="sm"
          fullWidth
          disabled={disabled}
          value={value.voice ?? voices[0] ?? ""}
          options={voices.map((v) => ({ value: v, label: v }))}
          onValueChange={(v: string) => set({ voice: v === voices[0] ? undefined : v })}
        />
      ) : voices ? (
        <Input
          label={t("parameterPanel.voice", "Voice")}
          size="sm"
          placeholder={t("parameterPanel.voicePlaceholder", "A voice name or id")}
          disabled={disabled}
          value={value.voice ?? ""}
          onChange={(e) => set({ voice: e.target.value.trim() === "" ? undefined : e.target.value })}
        />
      ) : null}
      {duration && (
        <NumberInput
          label={t("parameterPanel.duration", "Length in seconds")}
          size="sm"
          placeholder={modelDefault}
          align="start"
          min="0.5"
          step="0.5"
          value={value.durationSeconds === undefined ? "" : String(value.durationSeconds)}
          disabled={disabled}
          onValueChange={(v) => {
            const n = Number(v);
            set({ durationSeconds: v.trim() === "" || !Number.isFinite(n) || n <= 0 ? undefined : n });
          }}
        />
      )}
      {translate && (
        <div className="flex flex-col gap-8">
          <span className={cn("text-body font-medium", disabled ? "text-fg-disabled" : "text-fg")}>{t("parameterPanel.transcript", "Transcript")}</span>
          <Segmented
            aria-label={t("parameterPanel.transcript", "Transcript")}
            size="sm"
            fullWidth
            disabled={disabled}
            value={value.translate ? "english" : "spoken"}
            options={[
              { value: "spoken", label: t("parameterPanel.transcriptSpoken", "As spoken") },
              { value: "english", label: t("parameterPanel.transcriptEnglish", "In English") },
            ]}
            onValueChange={(v) => set({ translate: v === "english" ? true : undefined })}
          />
        </div>
      )}
      <div className="flex justify-end">
        <Button
          size="sm"
          variant="ghost"
          disabled={disabled || !changed}
          onClick={() => onValueChange(value.instructions !== undefined ? { instructions: value.instructions } : {})}
        >
          {t("parameterPanel.resetToDefaults", "Reset to defaults")}
        </Button>
      </div>
    </div>
  );
});
PanelBody.displayName = "ParameterPanel";

export const ParameterPanel = forwardRef<HTMLDivElement, ParameterPanelProps>(function ParameterPanel(
  { trigger, title: titleProp, open, defaultOpen, onOpenChange, side = "bottom", align = "end", ...panel },
  ref,
) {
  const t = useUiText();
  if (!trigger) return <PanelBody ref={ref} {...panel} />;
  const title = titleProp ?? t("parameterPopover.parameters", "Parameters");
  return (
    <Popover
      trapFocus
      {...(open !== undefined ? { open } : {})}
      {...(defaultOpen !== undefined ? { defaultOpen } : {})}
      {...(onOpenChange ? { onOpenChange } : {})}
    >
      <Popover.Trigger>{trigger}</Popover.Trigger>
      <Popover.Content ref={ref} aria-label={title} side={side} align={align} className="w-320 max-w-(--available-width)">
        <div className="relative">
          <p className="mb-16 flex min-h-32 items-center pr-40 text-body font-semibold text-fg">{title}</p>
          <PanelBody {...panel} layout="popover" />
          <div className="absolute top-0 right-0">
            <Popover.Close>
              <Button variant="ghost" icon={icons.close} aria-label={t("parameterPopover.close", "Close parameters")} size="sm" />
            </Popover.Close>
          </div>
        </div>
      </Popover.Content>
    </Popover>
  );
});
ParameterPanel.displayName = "ParameterPanel";
