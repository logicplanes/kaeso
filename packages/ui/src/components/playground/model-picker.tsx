import { forwardRef } from "react";
import { useUiText } from "../../i18n/context";
import { cn } from "../../utils/cn";
import { ModelLabel, providerName } from "../kaeso/identity";
import { Select } from "../select/select";

export const KAESO_MODEL_PICKER_VARIANTS = {
  appearance: {
    ghost: { classes: "", description: "Transparent until hovered: a column header (default)" },
    default: { classes: "", description: "A raised pill with a ring: a form or toolbar" },
  },
} as const;
export const KAESO_MODEL_PICKER_DEFAULT_VARIANTS = { appearance: "ghost" } as const;

export interface ModelPickerOption {
  value: string;
  provider: string;
  model?: string | undefined;
  name?: string | undefined;
  disabled?: boolean | undefined;
}

export interface ModelPickerProps {
  options: ModelPickerOption[];
  value?: string | null | undefined;
  defaultValue?: string | null | undefined;
  onValueChange?: ((value: string) => void) | undefined;
  groupByProvider?: boolean | undefined;
  placeholder?: string | undefined;
  "aria-label"?: string | undefined;
  appearance?: keyof typeof KAESO_MODEL_PICKER_VARIANTS.appearance | undefined;
  size?: "sm" | "md" | undefined;
  disabled?: boolean | undefined;
  "data-force"?: string | undefined;
  className?: string | undefined;
}

export const ModelPicker = forwardRef<HTMLButtonElement, ModelPickerProps>(function ModelPicker(
  {
    options,
    value,
    defaultValue,
    onValueChange,
    groupByProvider,
    placeholder,
    "aria-label": ariaLabel,
    appearance = "ghost",
    size = "sm",
    disabled,
    "data-force": force,
    className,
  },
  ref,
) {
  const t = useUiText();
  const grouped = groupByProvider ?? new Set(options.map((o) => o.provider)).size > 1;
  const sorted = grouped ? [...options].sort((a, b) => providerName(a.provider).localeCompare(providerName(b.provider))) : options;
  return (
    <Select
      ref={ref}
      aria-label={ariaLabel ?? t("modelPicker.model", "Model")}
      appearance={appearance}
      size={size}
      placeholder={placeholder ?? t("modelPicker.choose", "Choose a model")}
      {...(value !== undefined ? { value } : {})}
      {...(defaultValue !== undefined ? { defaultValue } : {})}
      {...(onValueChange ? { onValueChange } : {})}
      {...(disabled !== undefined ? { disabled } : {})}
      {...(force ? { "data-force": force } : {})}
      className={cn("max-w-full", appearance === "ghost" && "text-fg", className)}
      options={sorted.map((o) => ({
        value: o.value,
        textValue: o.name ?? o.model ?? o.value,
        label: <ModelLabel provider={o.provider} model={o.model ?? o.value} {...(o.name ? { name: o.name } : {})} />,
        ...(grouped ? { group: providerName(o.provider) } : {}),
        ...(o.disabled ? { disabled: true } : {}),
      }))}
    />
  );
});
ModelPicker.displayName = "ModelPicker";
