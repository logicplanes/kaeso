import { forwardRef, useMemo, useState, type ReactNode, type Ref } from "react";
import { Radio as BaseRadio } from "@base-ui/react/radio";
import { RadioGroup as BaseRadioGroup } from "@base-ui/react/radio-group";
import { cn } from "../../utils/cn.js";
import { SearchInput } from "../input/input.js";
import { useLocale, useUiText } from "../../i18n/context.js";
import { CommandList, type CommandListGroup } from "../command-list/command-list.js";
import { PickerPopover, PickerTrigger } from "../command-list/picker-popover.js";
import { useField, useFieldControl, withField, type FieldWrapProps } from "../field/field.js";
import { ProviderBadge, providerName } from "./identity.js";

export const KAESO_PROVIDER_PICKER_VARIANTS = {
  appearance: {
    list: { classes: "", description: "A search field over every provider, always open: a dialog step (default)" },
    select: { classes: "", description: "A pill that opens the searchable list: forms and toolbars" },
    grid: { classes: "", description: "Tiles with each provider's mark, connected ones first: the first step of a dialog" },
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
  onChoose?: ((id: string) => void) | undefined;
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
const SEARCH_ABOVE = 9;

function ProviderTiles({
  providers,
  value,
  onValueChange,
  onChoose,
  autoFocus,
  label,
  className,
}: {
  providers: ProviderPickerOption[];
  value: string | null | undefined;
  onValueChange: ((id: string | null) => void) | undefined;
  onChoose: ((id: string) => void) | undefined;
  autoFocus: boolean | undefined;
  label: string;
  className: string | undefined;
}) {
  const t = useUiText();
  const locale = useLocale();
  const [query, setQuery] = useState("");
  const wanted = query.trim().toLowerCase();
  const nameOf = (p: ProviderPickerOption) => providerName(p.id, p.name);
  const byName = (a: ProviderPickerOption, b: ProviderPickerOption) => nameOf(a).localeCompare(nameOf(b), locale, { sensitivity: "base" });
  const shown = providers.filter((p) => !wanted || [p.id, nameOf(p), ...(p.keywords ?? [])].some((k) => k.toLowerCase().includes(wanted)));
  const connected = shown.filter((p) => p.connected).sort(byName);
  const rest = shown.filter((p) => !p.connected).sort(byName);
  const sections =
    connected.length && rest.length
      ? [
          { id: "connected", label: t("providerPicker.connected", "Connected"), items: connected },
          { id: "other", label: t("providerPicker.other", "Other providers"), items: rest },
        ]
      : [{ id: "providers", label: undefined, items: [...connected, ...rest] }];
  const searchable = providers.length > SEARCH_ABOVE;
  return (
    <div className={cn("flex min-w-0 flex-col gap-16", className)}>
      {searchable && (
        <SearchInput
          aria-label={t("providerPicker.search", "Search providers")}
          placeholder={t("providerPicker.search", "Search providers")}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          {...(autoFocus ? { autoFocus: true } : {})}
        />
      )}
      {shown.length === 0 ? (
        <p className="py-24 text-center text-body text-fg-secondary">{t("providerPicker.noMatch", "No providers match")}</p>
      ) : (
        <BaseRadioGroup
          aria-label={label}
          {...(value ? { value } : {})}
          onValueChange={(v: unknown) => onValueChange?.(v ? String(v) : null)}
          className="flex flex-col gap-20"
        >
          {sections.map((section, si) => (
            <div key={section.id} {...(section.label ? { role: "group", "aria-label": section.label } : {})} className="flex flex-col gap-8">
              {section.label && (
                <div aria-hidden className="flex items-center justify-between text-caption font-medium text-fg-secondary">
                  <span>{section.label}</span>
                  <span className="font-regular text-fg-tertiary tabular">{section.items.length}</span>
                </div>
              )}
              <div className="grid grid-cols-2 gap-8 sm:grid-cols-3">
                {section.items.map((p, i) => (
                  <BaseRadio.Root
                    key={p.id}
                    value={p.id}
                    {...(p.disabled ? { disabled: true } : {})}
                    {...(autoFocus && !searchable && si === 0 && i === 0 && !value ? { autoFocus: true } : {})}
                    onClick={() => onChoose?.(p.id)}
                    className={cn(
                      "group/tile flex min-h-64 min-w-0 cursor-pointer items-center gap-10 rounded-group bg-raised px-10 py-12 sm:gap-12 sm:px-12 text-left shadow-control transition-control focus-ring",
                      "hover:shadow-control-hover data-checked:ring-step data-checked:bg-fill-1 data-disabled:cursor-not-allowed data-disabled:opacity-50",
                    )}
                  >
                    <span aria-hidden className="flex">
                      <ProviderBadge provider={p.id} name={p.name} size="lg" />
                    </span>
                    <span className="flex min-w-0 flex-1 flex-col">
                      <span className="truncate text-body font-medium text-fg">{nameOf(p)}</span>
                      {p.hint !== undefined && <span className="truncate text-small text-fg-secondary">{p.hint}</span>}
                    </span>
                  </BaseRadio.Root>
                ))}
              </div>
            </div>
          ))}
        </BaseRadioGroup>
      )}
    </div>
  );
}

export function useProviderGroups(providers: ProviderPickerOption[], allLabel?: string | undefined): CommandListGroup[] {
  const t = useUiText();
  const locale = useLocale();
  return useMemo(() => {
    const nameOf = (p: ProviderPickerOption) => providerName(p.id, p.name);
    const byName = (a: ProviderPickerOption, b: ProviderPickerOption) => nameOf(a).localeCompare(nameOf(b), locale, { sensitivity: "base" });
    const item = (p: ProviderPickerOption) => ({
      value: p.id,
      label: nameOf(p),
      keywords: [p.id, ...(p.keywords ?? [])],
      leading: (
        <span aria-hidden className="flex">
          <ProviderBadge provider={p.id} name={p.name} />
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
    onChoose,
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
  if (appearance === "grid") {
    const control = (
      <ProviderTiles
        providers={providers}
        value={value}
        onValueChange={onValueChange}
        onChoose={onChoose}
        autoFocus={autoFocus}
        label={listLabel}
        className={!(label || description || error) ? className : undefined}
      />
    );
    if (field) return control;
    return withField({ label, description, error, optional, required, layout, id: fc.wrapId, disabled, className }, control);
  }
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
                <ProviderBadge provider={chosen.id} name={chosen.name} />
              </span>
              <span className="truncate">{providerName(chosen.id, chosen.name)}</span>
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
