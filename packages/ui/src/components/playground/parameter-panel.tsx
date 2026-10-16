import { forwardRef, useId, type ReactElement } from "react";
import { cn } from "../../utils/cn.js";
import { icons } from "../../icons/vocabulary.js";
import { formatNumber } from "../../format/index.js";
import { Button } from "../button/button.js";
import { IconButton } from "../icon-button/icon-button.js";
import { NumberInput } from "../input/number-input.js";
import { Textarea } from "../input/textarea.js";
import { Popover } from "../popover/popover.js";
import { Segmented } from "../segmented/segmented.js";
import { Select } from "../select/select.js";
import { Slider } from "../slider/slider.js";
import { useUiText, type UiText } from "../../i18n/context.js";

export const KAESO_PARAMETER_PANEL_VARIANTS = {
  layout: {
    popover: { classes: "w-full gap-20", description: "Inside a 320px popover (ParameterPopover) (default)" },
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
}

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
  disabled?: boolean | undefined;
  layout?: keyof typeof KAESO_PARAMETER_PANEL_VARIANTS.layout | undefined;
  className?: string | undefined;
}

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
  const id = useId();
  const t = useUiText();
  const shown = formatNumber(value, { minimumFractionDigits: digits, maximumFractionDigits: digits });
  return (
    <div className="flex flex-col gap-8">
      <div className="flex items-baseline justify-between gap-12">
        <span id={id} className={cn("text-body font-medium", disabled ? "text-fg-disabled" : "text-fg")}>
          {label}
        </span>
        <span className={cn("text-small tabular", unset ? "text-fg-tertiary" : "text-fg-secondary")}>
          {unset ? t("parameterPanel.valueDefault", "{value} default", { value: shown }) : shown}
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

export const ParameterPanel = forwardRef<HTMLDivElement, ParameterPanelProps>(function ParameterPanel(
  {
    value,
    onValueChange,
    defaults,
    instructions = true,
    sampling = true,
    reasoning = false,
    maxOutputTokensLimit,
    disabled = false,
    layout = "popover",
    className,
  },
  ref,
) {
  const t = useUiText();
  const set = (patch: Partial<PlaygroundParameters>) => onValueChange({ ...value, ...patch });
  const all = efforts(t);
  const shownEfforts = reasoning === true ? all : Array.isArray(reasoning) ? all.filter((e) => reasoning.includes(e.value)) : [];
  const changed =
    value.temperature !== undefined ||
    value.topP !== undefined ||
    value.maxOutputTokens !== undefined ||
    value.reasoningEffort !== undefined ||
    value.reasoningSummary !== undefined;
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
      <NumberInput
        label={t("parameterPanel.maxOutputTokens", "Max output tokens")}
        size="sm"
        placeholder={t("parameterPanel.modelDefault", "Model default")}
        align="start"
        min="1"
        {...(maxOutputTokensLimit ? { max: String(maxOutputTokensLimit) } : {})}
        step="256"
        value={value.maxOutputTokens === undefined ? "" : String(value.maxOutputTokens)}
        disabled={disabled}
        onValueChange={(v) => {
          const n = Number(v);
          set({ maxOutputTokens: v.trim() === "" || !Number.isFinite(n) || n <= 0 ? undefined : Math.round(n) });
        }}
      />
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
ParameterPanel.displayName = "ParameterPanel";

export interface ParameterPopoverProps extends Omit<ParameterPanelProps, "layout" | "className"> {
  trigger?: ReactElement | undefined;
  title?: string | undefined;
  open?: boolean | undefined;
  defaultOpen?: boolean | undefined;
  onOpenChange?: ((open: boolean) => void) | undefined;
  side?: "top" | "bottom" | "left" | "right" | undefined;
  align?: "start" | "center" | "end" | undefined;
}

export function ParameterPopover({
  trigger,
  title: titleProp,
  open,
  defaultOpen,
  onOpenChange,
  side = "bottom",
  align = "end",
  ...panel
}: ParameterPopoverProps) {
  const t = useUiText();
  const title = titleProp ?? t("parameterPopover.parameters", "Parameters");
  return (
    <Popover
      trapFocus
      {...(open !== undefined ? { open } : {})}
      {...(defaultOpen !== undefined ? { defaultOpen } : {})}
      {...(onOpenChange ? { onOpenChange } : {})}
    >
      <Popover.Trigger>{trigger ?? <IconButton icon={icons.displayOptions} aria-label={title} size="sm" />}</Popover.Trigger>
      <Popover.Content title={title} side={side} align={align} className="w-320 max-w-(--available-width)">
        <ParameterPanel {...panel} />
      </Popover.Content>
    </Popover>
  );
}
ParameterPopover.displayName = "ParameterPopover";
