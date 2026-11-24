import { forwardRef, useCallback, useMemo, useState, type ReactNode } from "react";
import { useLocale, useUiText } from "../../i18n/context.js";
import { useFormatters } from "../../i18n/formatters.js";
import { cn } from "../../utils/cn.js";
import { figures } from "../../utils/figures.js";
import { CommandList, type CommandListGroup, type CommandListItem } from "../command-list/command-list.js";
import { PickerPopover, PickerTrigger } from "../command-list/picker-popover.js";
import { Highlight } from "../combobox/combobox.js";
import { Switch } from "../switch/switch.js";
import { KAESO_SELECT_VARIANTS, Select } from "../select/select.js";
import { itemMatch } from "../command-list/match.js";
import { useField, useFieldControl, withField, type FieldWrapProps } from "../field/field.js";
import { CapabilityIcons, ModelLabel, ProviderBadge, providerName, type Capability } from "../kaeso/identity.js";

const SELECT_APPEARANCE = { ghost: "header", default: "default" } as const;

export const KAESO_MODEL_PICKER_VARIANTS = {
  appearance: {
    ghost: { classes: KAESO_SELECT_VARIANTS.appearance.header.classes, description: "Transparent until hovered: a column header (default)" },
    default: { classes: KAESO_SELECT_VARIANTS.appearance.default.classes, description: "A pill with a text input's ring: a form or toolbar" },
  },
} as const;
export const KAESO_MODEL_PICKER_DEFAULT_VARIANTS = { appearance: "ghost" } as const;

export interface ModelPickerOption {
  value: string;
  provider: string;
  model?: string | undefined;
  name?: string | undefined;
  capabilities?: Capability[] | undefined;
  context?: number | undefined;
  price?: string | undefined;
  note?: string | undefined;
  deprecated?: boolean | undefined;
  disabled?: boolean | undefined;
}

export interface ModelPickerProvider {
  id: string;
  name?: string | undefined;
  connected?: boolean | undefined;
}

interface BaseProps extends FieldWrapProps {
  options: ModelPickerOption[];
  providers?: ModelPickerProvider[] | undefined;
  recent?: readonly string[] | undefined;
  groupByProvider?: boolean | undefined;
  placeholder?: string | undefined;
  "aria-label"?: string | undefined;
  appearance?: keyof typeof KAESO_MODEL_PICKER_VARIANTS.appearance | undefined;
  size?: "sm" | "md" | undefined;
  fullWidth?: boolean | undefined;
  disabled?: boolean | undefined;
  invalid?: boolean | undefined;
  open?: boolean | undefined;
  onOpenChange?: ((open: boolean) => void) | undefined;
  "data-force"?: string | undefined;
  className?: string | undefined;
}
export interface SingleModelPickerProps extends BaseProps {
  multiple?: false | undefined;
  allLabel?: string | undefined;
  value?: string | null | undefined;
  defaultValue?: string | null | undefined;
  onValueChange?: ((value: string) => void) | undefined;
}
export interface MultiModelPickerProps extends BaseProps {
  multiple: true;
  value?: string[] | undefined;
  defaultValue?: string[] | undefined;
  onValueChange?: ((value: string[]) => void) | undefined;
}
export type ModelPickerProps = SingleModelPickerProps | MultiModelPickerProps;

const ALL = "";
const ALL_PROVIDERS = "*";
// Defaults that keep their identity, so the memoized rows aren't rebuilt on every render.
const NONE: readonly string[] = [];
const NO_PROVIDERS: ModelPickerProvider[] = [];

const shows = (o: ModelPickerOption, kept: Set<string>, showDeprecated: boolean, only: string | null) =>
  (showDeprecated || !o.deprecated || kept.has(o.value)) && (!only || o.provider === only);

function ModelRow({ option, query, providerLabel }: { option: ModelPickerOption; query: string; providerLabel?: string | undefined }) {
  const { formatContext } = useFormatters();
  const model = option.model ?? option.value;
  // Rows redraw on every keystroke for their highlights. The mark, the capability tooltips and the details don't
  // change with the query: keeping their elements lets React skip them, and formatting runs once per row.
  const { badge, end } = useMemo(() => {
    const details = option.note ?? [option.context ? formatContext(option.context) : "", option.price ?? ""].filter(Boolean).join(" · ");
    return {
      badge: providerLabel !== undefined && (
        <span aria-hidden className="flex">
          <ProviderBadge provider={option.provider} name={providerLabel} model={model} />
        </span>
      ),
      end: (
        <>
          {option.capabilities && option.capabilities.length > 0 && <CapabilityIcons capabilities={option.capabilities} className="hidden sm:flex" />}
          {details && (
            <span
              className={cn("shrink-0 text-caption font-regular", figures(details), option.note ? "text-fg-secondary" : "hidden text-fg-tertiary sm:inline")}
            >
              {details}
            </span>
          )}
        </>
      ),
    };
  }, [option, providerLabel, model, formatContext]);
  return (
    <>
      {badge}
      <span className="flex min-w-0 flex-1 items-baseline gap-8">
        <span className="min-w-0 shrink truncate">
          <Highlight text={option.name ?? model} query={query} />
        </span>
        {option.name && option.name !== model && (
          <span className="min-w-0 flex-1 truncate mono text-small font-regular text-fg-tertiary">
            <Highlight text={model} query={query} />
          </span>
        )}
      </span>
      {end}
    </>
  );
}

function modelItem(o: ModelPickerOption, nameOf: (provider: string) => string, withBadge: boolean): CommandListItem {
  const model = o.model ?? o.value;
  const providerLabel = nameOf(o.provider);
  return {
    value: o.value,
    label: o.name ?? model,
    keywords: [model, providerLabel, o.provider],
    render: ({ query: q }) => <ModelRow option={o} query={q} {...(withBadge ? { providerLabel } : {})} />,
    ...(o.disabled ? { disabled: true } : {}),
  };
}

function modelGroups(
  options: ModelPickerOption[],
  providerOrder: string[],
  grouped: boolean,
  nameOf: (provider: string) => string,
  itemOf: (o: ModelPickerOption) => CommandListItem = (o) => modelItem(o, nameOf, !grouped),
): CommandListGroup[] {
  if (!grouped) return [{ id: "models", items: options.map(itemOf) }];
  const byProvider = new Map<string, ModelPickerOption[]>();
  for (const o of options) {
    const models = byProvider.get(o.provider);
    if (models) models.push(o);
    else byProvider.set(o.provider, [o]);
  }
  return providerOrder.flatMap((id) => {
    const models = byProvider.get(id);
    if (!models) return [];
    return [
      {
        id: `provider:${id}`,
        label: nameOf(id),
        leading: (
          <span aria-hidden className="flex">
            <ProviderBadge provider={id} name={nameOf(id)} size="sm" />
          </span>
        ),
        count: models.length,
        items: models.map(itemOf),
      },
    ];
  });
}

function useProviderOrder(options: ModelPickerOption[], providers: ModelPickerProvider[]) {
  const locale = useLocale();
  const provider = useMemo(() => new Map(providers.map((p) => [p.id, p])), [providers]);
  const nameOf = useCallback((id: string) => providerName(id, provider.get(id)?.name), [provider]);
  const providerOrder = useMemo(() => {
    const ids = [...new Set(options.map((o) => o.provider))];
    return ids.sort(
      (a, b) =>
        Number(!!provider.get(b)?.connected) - Number(!!provider.get(a)?.connected) || nameOf(a).localeCompare(nameOf(b), locale, { sensitivity: "base" }),
    );
  }, [options, provider, nameOf, locale]);
  return { nameOf, providerOrder };
}

export function useModelGroups(
  options: ModelPickerOption[],
  { providers = NO_PROVIDERS, groupByProvider }: { providers?: ModelPickerProvider[] | undefined; groupByProvider?: boolean | undefined } = {},
): CommandListGroup[] {
  const { nameOf, providerOrder } = useProviderOrder(options, providers);
  const grouped = groupByProvider ?? providerOrder.length > 1;
  return useMemo(() => modelGroups(options, providerOrder, grouped, nameOf), [options, providerOrder, grouped, nameOf]);
}

export const ModelPicker = forwardRef<HTMLButtonElement, ModelPickerProps>(function ModelPicker(props, ref) {
  const {
    options,
    providers = NO_PROVIDERS,
    recent = NONE,
    groupByProvider,
    placeholder,
    "aria-label": ariaLabel,
    appearance = "ghost",
    size = "sm",
    fullWidth,
    disabled,
    invalid,
    open: openProp,
    onOpenChange,
    "data-force": force,
    className,
    label,
    description,
    error,
    optional,
    required,
    layout,
  } = props;
  const t = useUiText();
  const { formatNumber } = useFormatters();
  const field = useField();
  const fc = useFieldControl(props);
  const [ownOpen, setOwnOpen] = useState(false);
  const open = openProp ?? ownOpen;
  const setOpen = (o: boolean) => {
    if (openProp === undefined) setOwnOpen(o);
    onOpenChange?.(o);
  };
  const [own, setOwn] = useState<string | string[] | null>(() => props.defaultValue ?? (props.multiple ? [] : null));
  const current = props.value !== undefined ? props.value : own;
  const chosen = Array.isArray(current) ? current : current ? [current] : [];
  const allLabel = props.multiple ? undefined : props.allLabel;
  const [query, setQuery] = useState("");
  const [only, setOnly] = useState<string | null>(null);
  const [showDeprecated, setShowDeprecated] = useState(false);
  const [wasOpen, setWasOpen] = useState(open);
  if (open !== wasOpen) {
    setWasOpen(open);
    if (open) {
      setQuery("");
      setOnly(null);
    }
  }

  const byValue = useMemo(() => new Map(options.map((o) => [o.value, o])), [options]);
  const { nameOf, providerOrder } = useProviderOrder(options, providers);
  const grouped = groupByProvider ?? providerOrder.length > 1;
  const hasDeprecated = options.some((o) => o.deprecated && !chosen.includes(o.value));

  const chosenKey = chosen.join("\u0000");
  const searching = query.trim() !== "";
  // Built once while the list is open, not on every keystroke: the list filters these same items, and their
  // match scores are kept per item, so the provider counts and the list score each model only once.
  const items = useMemo(() => (open ? new Map(options.map((o) => [o, modelItem(o, nameOf, !grouped)])) : null), [open, options, nameOf, grouped]);
  const listed = useMemo<CommandListGroup[]>(() => {
    if (!items) return [];
    const kept = new Set(chosenKey.split("\u0000"));
    const shown = options.filter((o) => shows(o, kept, showDeprecated, only));
    return modelGroups(shown, providerOrder, grouped, nameOf, (o) => items.get(o) ?? modelItem(o, nameOf, !grouped));
  }, [items, options, providerOrder, grouped, nameOf, showDeprecated, chosenKey, only]);
  const head = useMemo<CommandListGroup[]>(() => {
    if (!open) return [];
    const kept = new Set(chosenKey.split("\u0000"));
    const out: CommandListGroup[] = [];
    if (allLabel && !only) out.push({ id: "all", items: [{ value: ALL, label: allLabel }] });
    const recentItems =
      searching || only
        ? []
        : recent.flatMap((v) => {
            const o = byValue.get(v);
            return o && shows(o, kept, showDeprecated, only) ? [modelItem(o, nameOf, true)] : [];
          });
    if (recentItems.length) out.push({ id: "recent", label: t("modelPicker.recent", "Recent"), items: recentItems });
    return out;
  }, [open, recent, byValue, searching, only, showDeprecated, chosenKey, allLabel, t, nameOf]);
  const groups = useMemo(() => (head.length ? [...head, ...listed] : listed), [head, listed]);

  const pick = (v: string) => {
    if (props.multiple) {
      const next = chosen.includes(v) ? chosen.filter((x) => x !== v) : [...chosen, v];
      if (props.value === undefined) setOwn(next);
      props.onValueChange?.(next);
      return;
    }
    if (props.value === undefined) setOwn(v === ALL ? null : v);
    props.onValueChange?.(v);
    setOpen(false);
  };

  const shownProviders = useMemo(() => {
    if (!items) return [];
    const q = query.trim();
    const kept = new Set(chosenKey.split("\u0000"));
    const counts = new Map<string, number>();
    for (const [o, item] of items) {
      if (!shows(o, kept, showDeprecated, null)) continue;
      if (q && itemMatch(item, q) <= 0) continue;
      counts.set(o.provider, (counts.get(o.provider) ?? 0) + 1);
    }
    return providerOrder.filter((id) => counts.has(id) || id === only).map((id) => ({ id, count: counts.get(id) ?? 0 }));
  }, [items, providerOrder, query, showDeprecated, chosenKey, only]);
  const providerFilter =
    grouped && providerOrder.length > 1 ? (
      <Select
        aria-label={t("modelPicker.provider", "Provider")}
        size="sm"
        appearance="ghost"
        value={only ?? ALL_PROVIDERS}
        onValueChange={(v: string) => setOnly(v === ALL_PROVIDERS ? null : v)}
        options={[
          { value: ALL_PROVIDERS, label: t("modelPicker.everyProvider", "All providers") },
          ...shownProviders.map(({ id, count }) => ({
            value: id,
            textValue: nameOf(id),
            label: (
              <span className="flex min-w-0 items-center gap-8">
                <span aria-hidden className="flex shrink-0">
                  <ProviderBadge provider={id} size="sm" />
                </span>
                <span className="min-w-0 truncate">{nameOf(id)}</span>
                <span className="shrink-0 text-fg-tertiary tabular">{formatNumber(count)}</span>
              </span>
            ),
          })),
        ]}
      />
    ) : null;
  const toolbar =
    providerFilter || hasDeprecated ? (
      <>
        {providerFilter}
        {hasDeprecated && (
          <Switch size="sm" label={t("modelPicker.showDeprecated", "Show deprecated")} checked={showDeprecated} onCheckedChange={setShowDeprecated} />
        )}
      </>
    ) : undefined;

  const first = chosen[0] ? byValue.get(chosen[0]) : undefined;
  const summary = props.multiple ? (
    first ? (
      chosen.length === 1 ? (
        <ModelLabel
          provider={first.provider}
          providerName={nameOf(first.provider)}
          model={first.model ?? first.value}
          {...(first.name ? { name: first.name } : {})}
        />
      ) : (
        <span className="flex min-w-0 items-center">
          <span className="min-w-0 truncate">{first.name ?? first.model ?? first.value}</span>{" "}
          <span className="shrink-0 pl-4 tabular">{t("select.more", "+{count}", { count: String(chosen.length - 1) })}</span>
        </span>
      )
    ) : undefined
  ) : first ? (
    <ModelLabel
      provider={first.provider}
      providerName={nameOf(first.provider)}
      model={first.model ?? first.value}
      {...(first.name ? { name: first.name } : {})}
    />
  ) : current === ALL || (current === null && allLabel) ? (
    allLabel
  ) : undefined;
  const title = ariaLabel ?? (typeof label === "string" ? label : t("modelPicker.model", "Model"));

  const control = (
    <PickerPopover
      title={title}
      width="lg"
      open={open}
      onOpenChange={setOpen}
      trigger={
        <PickerTrigger
          ref={ref}
          appearance={SELECT_APPEARANCE[appearance]}
          size={size}
          {...fc.controlProps}
          {...(ariaLabel || !label ? { "aria-label": title } : {})}
          {...(disabled ? { disabled } : {})}
          {...(invalid ? { invalid } : {})}
          {...(fullWidth ? { fullWidth } : {})}
          {...(force ? { "data-force": force } : {})}
          {...(first && !props.multiple
            ? {
                title:
                  first.name && first.name !== (first.model ?? first.value) ? `${first.name} · ${first.model ?? first.value}` : (first.model ?? first.value),
              }
            : {})}
          placeholder={placeholder ?? (props.multiple ? t("modelPicker.chooseSome", "Choose models") : t("modelPicker.choose", "Choose a model"))}
          className={cn("max-w-full", !(label || description || error) && className)}
        >
          {summary as ReactNode}
        </PickerTrigger>
      }
    >
      <CommandList
        groups={groups}
        aria-label={title}
        value={props.multiple ? chosen : (chosen[0] ?? (allLabel ? ALL : null))}
        {...(props.multiple ? { multiple: true } : {})}
        onSelect={pick}
        query={query}
        onQueryChange={setQuery}
        placeholder={t("modelPicker.search", "Search models")}
        emptyText={t("modelPicker.noMatch", "No models match")}
        toolbar={toolbar}
        autoFocus
      />
    </PickerPopover>
  );
  if (field) return control;
  return withField({ label, description, error, optional, required, layout, id: fc.wrapId, disabled, className }, control);
});
ModelPicker.displayName = "ModelPicker";
