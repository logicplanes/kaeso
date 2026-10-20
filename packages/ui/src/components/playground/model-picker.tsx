import { forwardRef, useCallback, useMemo, useState, type ReactNode } from "react";
import { useLocale, useUiText } from "../../i18n/context.js";
import { cn } from "../../utils/cn.js";
import { formatTokens } from "../../format/index.js";
import { CommandList, type CommandListGroup, type CommandListItem } from "../command-list/command-list.js";
import { PickerPopover, PickerTrigger } from "../command-list/picker-popover.js";
import { Highlight } from "../combobox/combobox.js";
import { Button } from "../button/button.js";
import { Switch } from "../switch/switch.js";
import { KAESO_SELECT_VARIANTS } from "../select/select.js";
import { useField, useFieldControl, withField, type FieldWrapProps } from "../field/field.js";
import { CapabilityIcons, ModelLabel, ProviderBadge, providerName, type Capability } from "../kaeso/identity.js";

const SELECT_APPEARANCE = { ghost: "header", default: "default" } as const;

export const KAESO_MODEL_PICKER_VARIANTS = {
  appearance: {
    ghost: { classes: KAESO_SELECT_VARIANTS.appearance.header.classes, description: "Transparent until hovered: a column header (default)" },
    default: { classes: KAESO_SELECT_VARIANTS.appearance.default.classes, description: "A raised pill with a ring: a form or toolbar" },
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
const CHIPS_ABOVE = 1;

function ModelRow({ option, query, providerLabel }: { option: ModelPickerOption; query: string; providerLabel?: string | undefined }) {
  const model = option.model ?? option.value;
  const details = [option.context ? formatTokens(option.context) : "", option.price ?? ""].filter(Boolean).join(" · ");
  return (
    <>
      {providerLabel !== undefined && (
        <span aria-hidden className="flex">
          <ProviderBadge provider={option.provider} name={providerLabel} />
        </span>
      )}
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
      {option.capabilities && option.capabilities.length > 0 && <CapabilityIcons capabilities={option.capabilities} className="hidden sm:flex" />}
      {(option.note ?? details) && (
        <span className={cn("shrink-0 text-caption font-regular tabular", option.note ? "text-fg-secondary" : "hidden text-fg-tertiary sm:inline")}>
          {option.note ?? details}
        </span>
      )}
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

function modelGroups(options: ModelPickerOption[], providerOrder: string[], grouped: boolean, nameOf: (provider: string) => string): CommandListGroup[] {
  if (!grouped) return [{ id: "models", items: options.map((o) => modelItem(o, nameOf, true)) }];
  return providerOrder.flatMap((id) => {
    const models = options.filter((o) => o.provider === id);
    if (!models.length) return [];
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
        items: models.map((o) => modelItem(o, nameOf, false)),
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
  { providers = [], groupByProvider }: { providers?: ModelPickerProvider[] | undefined; groupByProvider?: boolean | undefined } = {},
): CommandListGroup[] {
  const { nameOf, providerOrder } = useProviderOrder(options, providers);
  const grouped = groupByProvider ?? providerOrder.length > 1;
  return useMemo(() => modelGroups(options, providerOrder, grouped, nameOf), [options, providerOrder, grouped, nameOf]);
}

export const ModelPicker = forwardRef<HTMLButtonElement, ModelPickerProps>(function ModelPicker(props, ref) {
  const {
    options,
    providers = [],
    recent = [],
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

  const byValue = useMemo(() => new Map(options.map((o) => [o.value, o])), [options]);
  const { nameOf, providerOrder } = useProviderOrder(options, providers);
  const grouped = groupByProvider ?? providerOrder.length > 1;
  const hasDeprecated = options.some((o) => o.deprecated && !chosen.includes(o.value));

  const chosenKey = chosen.join("\u0000");
  const groups = useMemo<CommandListGroup[]>(() => {
    const kept = new Set(chosenKey.split("\u0000"));
    const visible = (o: ModelPickerOption) => (showDeprecated || !o.deprecated || kept.has(o.value)) && (!only || o.provider === only);
    const out: CommandListGroup[] = [];
    if (allLabel && !only) out.push({ id: "all", items: [{ value: ALL, label: allLabel }] });
    const recentItems =
      query.trim() || only ? [] : recent.flatMap((v) => (byValue.get(v) && visible(byValue.get(v)!) ? [modelItem(byValue.get(v)!, nameOf, true)] : []));
    if (recentItems.length) out.push({ id: "recent", label: t("modelPicker.recent", "Recent"), items: recentItems });
    return [...out, ...modelGroups(options.filter(visible), providerOrder, grouped, nameOf)];
  }, [options, providerOrder, grouped, recent, byValue, query, only, showDeprecated, chosenKey, allLabel, t, nameOf]);

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

  const chips =
    grouped && providerOrder.length > CHIPS_ABOVE ? (
      <div role="group" aria-label={t("modelPicker.providers", "Providers")} className="flex min-w-0 flex-1 gap-4 overflow-x-auto scrollbar-none">
        <Button size="xs" variant={only ? "ghost" : "secondary"} aria-pressed={!only} onClick={() => setOnly(null)}>
          {t("modelPicker.allProviders", "All")}
        </Button>
        {providerOrder.map((id) => (
          <Button key={id} size="xs" variant={only === id ? "secondary" : "ghost"} aria-pressed={only === id} onClick={() => setOnly(only === id ? null : id)}>
            {nameOf(id)}
          </Button>
        ))}
      </div>
    ) : null;
  const toolbar =
    chips || hasDeprecated ? (
      <>
        {chips}
        {hasDeprecated && (
          <Switch size="sm" label={t("modelPicker.showDeprecated", "Deprecated")} checked={showDeprecated} onCheckedChange={setShowDeprecated} />
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
        t("modelPicker.firstAndMore", "{first} +{count}", { first: first.name ?? first.model ?? first.value, count: String(chosen.length - 1) })
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
      onOpenChange={(o) => {
        setOpen(o);
        if (!o) setQuery("");
      }}
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
