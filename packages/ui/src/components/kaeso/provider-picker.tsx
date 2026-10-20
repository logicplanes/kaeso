import { forwardRef, useMemo, useState, type ReactNode, type Ref } from "react";
import { useLocale, useUiText } from "../../i18n/context.js";
import { CommandList, type CommandListGroup } from "../command-list/command-list.js";
import { PickerPopover, PickerTrigger } from "../command-list/picker-popover.js";
import { useField, useFieldControl, withField, type FieldWrapProps } from "../field/field.js";
import { ProviderBadge, providerName } from "./identity.js";

export const KAESO_PROVIDER_PICKER_VARIANTS = {
  appearance: {
    list: { classes: "", description: "A search field over every provider, always open: a dialog step (default)" },
    select: { classes: "", description: "A pill that opens the searchable list: forms and toolbars" },
  },
} as const;
export const KAESO_PROVIDER_PICKER_DEFAULT_VARIANTS = { appearance: "list" } as const;

export interface ProviderPickerOption {
  id: string;
  name?: string | undefined;
  connected?: boolean | undefined;
  hint?: ReactNode | undefined;
  keywords?: readonly string[] | undefined;
  disabled?: boolean | undefined;
}

export interface ProviderPickerProps extends FieldWrapProps {
  providers: ProviderPickerOption[];
  value?: string | null | undefined;
  onValueChange?: ((id: string | null) => void) | undefined;
  appearance?: keyof typeof KAESO_PROVIDER_PICKER_VARIANTS.appearance | undefined;
  allLabel?: string | undefined;
  placeholder?: string | undefined;
  size?: "sm" | "md" | undefined;
  disabled?: boolean | undefined;
  autoFocus?: boolean | undefined;
  "aria-label"?: string | undefined;
  className?: string | undefined;
  "data-force"?: string | undefined;
}

const ALL = "";

export function useProviderGroups(providers: ProviderPickerOption[], allLabel?: string | undefined): CommandListGroup[] {
  const t = useUiText();
  const locale = useLocale();
  return useMemo(() => {
    const nameOf = (p: ProviderPickerOption) => p.name ?? providerName(p.id);
    const byName = (a: ProviderPickerOption, b: ProviderPickerOption) => nameOf(a).localeCompare(nameOf(b), locale, { sensitivity: "base" });
    const item = (p: ProviderPickerOption) => ({
      value: p.id,
      label: nameOf(p),
      keywords: [p.id, ...(p.keywords ?? [])],
      leading: (
        <span aria-hidden className="flex">
          <ProviderBadge provider={p.id} />
        </span>
      ),
      ...(p.hint !== undefined ? { meta: p.hint } : {}),
      ...(p.disabled ? { disabled: true } : {}),
    });
    const connected = providers.filter((p) => p.connected).sort(byName);
    const rest = providers.filter((p) => !p.connected).sort(byName);
    const groups: CommandListGroup[] = [];
    if (allLabel) groups.push({ id: "all", items: [{ value: ALL, label: allLabel }] });
    if (connected.length && rest.length) {
      groups.push({ id: "connected", label: t("providerPicker.connected", "Connected"), count: connected.length, items: connected.map(item) });
      groups.push({ id: "other", label: t("providerPicker.other", "Other providers"), count: rest.length, items: rest.map(item) });
    } else groups.push({ id: "providers", items: [...connected, ...rest].map(item) });
    return groups;
  }, [providers, allLabel, t, locale]);
}

export const ProviderPicker = forwardRef<HTMLElement, ProviderPickerProps>(function ProviderPicker(props, ref) {
  const {
    providers,
    value,
    onValueChange,
    appearance = "list",
    allLabel,
    placeholder,
    size = "md",
    disabled,
    autoFocus,
    "aria-label": ariaLabel,
    className,
    "data-force": force,
    label,
    description,
    error,
    optional,
    required,
    layout,
  } = props;
  const t = useUiText();
  const field = useField();
  const fc = useFieldControl(props);
  const [open, setOpen] = useState(false);
  const groups = useProviderGroups(providers, appearance === "select" ? allLabel : undefined);
  const listLabel = ariaLabel ?? (typeof label === "string" ? label : t("providerPicker.providers", "Providers"));
  const search = placeholder ?? t("providerPicker.search", "Search providers");
  const empty = t("providerPicker.noMatch", "No providers match");
  const list = (height?: "inline") => (
    <CommandList
      groups={groups}
      aria-label={listLabel}
      value={value ?? (allLabel ? ALL : null)}
      onSelect={(id) => {
        onValueChange?.(id === ALL ? null : id);
        setOpen(false);
      }}
      placeholder={search}
      emptyText={empty}
      {...(height ? { height } : {})}
      {...(autoFocus || appearance === "select" ? { autoFocus: true } : {})}
      {...(height && force ? { "data-force": force } : {})}
      {...(height && className && !(label || description || error) ? { className } : {})}
      {...(height ? { ref: ref as Ref<HTMLInputElement> } : {})}
    />
  );
  if (appearance === "list") {
    const control = list("inline");
    if (field) return control;
    return withField({ label, description, error, optional, required, layout, id: fc.wrapId, disabled, className }, control);
  }
  const chosen = providers.find((p) => p.id === value);
  const control = (
    <PickerPopover
      title={listLabel}
      width="sm"
      open={open}
      onOpenChange={setOpen}
      trigger={
        <PickerTrigger
          ref={ref as Ref<HTMLButtonElement>}
          size={size === "sm" ? "sm" : "md"}
          {...fc.controlProps}
          {...(ariaLabel ? { "aria-label": ariaLabel } : {})}
          {...(disabled ? { disabled } : {})}
          {...(force ? { "data-force": force } : {})}
          placeholder={placeholder ?? t("providerPicker.choose", "Choose a provider")}
          {...(!(label || description || error) && className ? { className } : {})}
        >
          {chosen ? (
            <span className="flex min-w-0 items-center gap-8">
              <span aria-hidden className="flex">
                <ProviderBadge provider={chosen.id} />
              </span>
              <span className="truncate">{chosen.name ?? providerName(chosen.id)}</span>
            </span>
          ) : (
            allLabel
          )}
        </PickerTrigger>
      }
    >
      {list()}
    </PickerPopover>
  );
  if (field) return control;
  return withField({ label, description, error, optional, required, layout, id: fc.wrapId, disabled, className }, control);
});
ProviderPicker.displayName = "ProviderPicker";
