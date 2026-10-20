import { pageBoundary, usePortalContainer } from "../../utils/portal-context.js";
import { forwardRef, useEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { Combobox as BaseCombobox } from "@base-ui/react/combobox";
import { Autocomplete as BaseAutocomplete } from "@base-ui/react/autocomplete";
import { cn } from "../../utils/cn.js";
import { warnStylingClassName } from "../../utils/dev.js";
import { useField, useFieldControl, withField, type FieldWrapProps } from "../field/field.js";
import { Icon, type IconData } from "../icon/icon.js";
import { icons } from "../../icons/vocabulary.js";
import { KAESO_INPUT_GROUP_VARIANTS, type InputSize } from "../input/input-group.js";
import { Tag } from "../tag/tag.js";
import { MENU_ICON, MENU_ITEM, MENU_POPUP } from "../menu/styles.js";

import { anchorWidthStyle, useLayoutWidth } from "../../hooks/use-layout-width.js";
import { useUiText } from "../../i18n/context.js";

export const KAESO_COMBOBOX_VARIANTS = {
  size: {
    sm: {
      classes: KAESO_INPUT_GROUP_VARIANTS.size.sm.classes,
      description: "32px",
    },
    md: {
      classes: KAESO_INPUT_GROUP_VARIANTS.size.md.classes,
      description: "36px (default)",
    },
    lg: {
      classes: KAESO_INPUT_GROUP_VARIANTS.size.lg.classes,
      description: "40px",
    },
    xl: {
      classes: KAESO_INPUT_GROUP_VARIANTS.size.xl.classes,
      description: "48px",
    },
  },
} as const;
export const KAESO_COMBOBOX_DEFAULT_VARIANTS = { size: "md" } as const;

export interface ComboboxOption {
  value: string;
  label: string;
  description?: ReactNode | undefined;
  icon?: IconData | undefined;
  leading?: ReactNode | undefined;
  disabled?: boolean | undefined;
}

interface CreateItem extends ComboboxOption {
  __create: true;
}

export function Highlight({ text, query }: { text: string; query: string }) {
  const q = query.trim().toLowerCase();
  if (!q) return <>{text}</>;
  const i = text.toLowerCase().indexOf(q);
  if (i < 0) return <span className="text-fg-secondary">{text}</span>;
  return (
    <span className="text-fg-secondary">
      {text.slice(0, i)}
      <span className="text-fg">{text.slice(i, i + q.length)}</span>
      {text.slice(i + q.length)}
    </span>
  );
}

interface BaseProps extends FieldWrapProps {
  options: ComboboxOption[];
  placeholder?: string | undefined;
  size?: InputSize | undefined;
  emptyText?: ReactNode | undefined;
  loading?: boolean | undefined;
  onInputValueChange?: ((value: string) => void) | undefined;
  searchDelay?: number | undefined;
  onCreate?: ((label: string) => void) | undefined;
  createLabel?: ((label: string) => string) | undefined;
  disabled?: boolean | undefined;
  invalid?: boolean | undefined;
  id?: string | undefined;
  name?: string | undefined;
  "aria-label"?: string | undefined;
  className?: string | undefined;
  "data-force"?: string | undefined;
}
export interface SingleComboboxProps extends BaseProps {
  multiple?: false | undefined;
  value?: string | null | undefined;
  defaultValue?: string | null | undefined;
  onValueChange?: ((value: string | null) => void) | undefined;
}
export interface MultiComboboxProps extends BaseProps {
  multiple: true;
  value?: string[] | undefined;
  defaultValue?: string[] | undefined;
  onValueChange?: ((value: string[]) => void) | undefined;
}
export type ComboboxProps = SingleComboboxProps | MultiComboboxProps;

function keepValueOnEscape(e: KeyboardEvent<HTMLInputElement> & { preventBaseUIHandler?: (() => void) | undefined }) {
  if (e.key === "Escape" && e.currentTarget.getAttribute("aria-expanded") !== "true") e.preventBaseUIHandler?.();
}

export const Combobox = forwardRef<HTMLInputElement, ComboboxProps>(function Combobox(props, ref) {
  const {
    options,
    placeholder,
    size = "md",
    emptyText,
    loading,
    onInputValueChange,
    searchDelay = 0,
    onCreate,
    createLabel,
    disabled,
    invalid,
    id,
    name,
    label,
    description,
    error,
    optional,
    required,
    layout,
    className,
    multiple,
    "aria-label": ariaLabel,
    "data-force": force,
  } = props;
  warnStylingClassName("Combobox", className);
  const t = useUiText();
  const field = useField();
  const fc = useFieldControl(props);
  const [query, setQuery] = useState("");
  const fieldRef = useRef<HTMLDivElement | null>(null);
  const fieldWidth = useLayoutWidth(fieldRef);
  const [inner, setInner] = useState<string | null | string[]>(() => props.defaultValue ?? (multiple ? [] : null));
  const current = props.value !== undefined ? props.value : inner;
  const chosenKey = (Array.isArray(current) ? current : current == null ? [] : [current]).join("\u0000");
  const seen = useRef(new Map<string, ComboboxOption>());
  const byValue = useMemo(() => {
    const chosen = new Set(chosenKey ? chosenKey.split("\u0000") : []);
    const next = new Map<string, ComboboxOption>();
    for (const [v, o] of seen.current) if (chosen.has(v)) next.set(v, o);
    for (const o of options) next.set(o.value, o);
    seen.current = next;
    return next;
  }, [options, chosenKey]);
  const { filtered, hidden } = useMemo(() => {
    const SHOWN_AT_MOST = 100;
    const q = query.trim().toLowerCase();
    const matched = q ? options.filter((o) => o.label.toLowerCase().includes(q) || o.value.toLowerCase().includes(q)) : options;
    const list: (ComboboxOption | CreateItem)[] = matched.slice(0, SHOWN_AT_MOST);
    if (onCreate && q && !options.some((o) => o.label.toLowerCase() === q))
      list.push({
        value: `__create__${query}`,
        label: query.trim(),
        __create: true,
      });
    return { filtered: list, hidden: Math.max(0, matched.length - SHOWN_AT_MOST) };
  }, [options, query, onCreate]);
  const report = useRef(onInputValueChange);
  report.current = onInputValueChange;
  const pending = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    const timer = pending;
    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
  }, []);
  const reportInput = (v: string) => {
    if (searchDelay <= 0) return report.current?.(v);
    if (pending.current) clearTimeout(pending.current);
    pending.current = setTimeout(() => {
      pending.current = null;
      report.current?.(v);
    }, searchDelay);
  };
  const rootValue = Array.isArray(current) ? current.flatMap((v) => byValue.get(v) ?? []) : current == null ? null : (byValue.get(current) ?? null);

  const handleChange = (v: unknown) => {
    const arr = Array.isArray(v) ? (v as (ComboboxOption | CreateItem)[]) : null;
    const created = (arr ?? [v as ComboboxOption | CreateItem | null]).find((x) => x && "__create" in x) as CreateItem | undefined;
    if (created) {
      onCreate?.(created.label);
      return;
    }
    const next = multiple ? (arr ?? []).map((x) => x.value) : ((v as ComboboxOption | null)?.value ?? null);
    if (props.value === undefined) setInner(next);
    if (multiple) (props.onValueChange as ((x: string[]) => void) | undefined)?.(next as string[]);
    else (props.onValueChange as ((x: string | null) => void) | undefined)?.(next as string | null);
  };

  const inputEl = (
    <BaseCombobox.Input
      ref={ref}
      placeholder={placeholder}
      {...(id ? { id } : {})}
      {...fc.controlProps}
      {...(ariaLabel ? { "aria-label": ariaLabel } : {})}
      {...(invalid || error ? { "aria-invalid": true } : {})}
      onKeyDown={keepValueOnEscape}
      className="min-w-80 flex-1 bg-transparent text-fg outline-none placeholder:text-fg-tertiary disabled:cursor-not-allowed"
    />
  );

  const control = (
    <BaseCombobox.Root
      items={filtered}
      filteredItems={filtered}
      value={rootValue}
      onValueChange={handleChange}
      onInputValueChange={(v: string) => {
        setQuery(v);
        reportInput(v);
      }}
      autoHighlight
      itemToStringLabel={(o: ComboboxOption) => o?.label ?? ""}
      isItemEqualToValue={(a: ComboboxOption, b: ComboboxOption) => a?.value === b?.value}
      multiple={multiple === true}
      {...(disabled ? { disabled } : {})}
      {...(name ? { name } : {})}
    >
      {multiple ? (
        <BaseCombobox.Chips
          ref={fieldRef}
          {...(force ? { "data-force": force } : {})}
          className={cn(
            "flex min-h-36 w-full flex-wrap items-center gap-4 rounded-field p-4 material-field transition-control cursor-text",
            (invalid || error) && "ring-invalid",
            !(label || description || error) && className,
          )}
        >
          <BaseCombobox.Value>
            {(value: ComboboxOption[]) => (
              <>
                {value.map((o) => (
                  <BaseCombobox.Chip key={o.value} aria-label={o.label} className="outline-none">
                    <Tag className="pr-2">
                      {o.label}
                      <BaseCombobox.ChipRemove
                        aria-label={t("combobox.remove", "Remove {label}", { label: o.label })}
                        className="ml-2 inline-flex size-24 items-center justify-center rounded-pill text-fg-tertiary transition-control hover:bg-fill-2 hover:text-fg"
                      >
                        <Icon icon={icons.close} size="xs" />
                      </BaseCombobox.ChipRemove>
                    </Tag>
                  </BaseCombobox.Chip>
                ))}
                <span className={cn("flex h-28 min-w-80 flex-1 items-center", value.length === 0 ? "px-12" : "px-8")}>{inputEl}</span>
              </>
            )}
          </BaseCombobox.Value>
        </BaseCombobox.Chips>
      ) : (
        <BaseCombobox.InputGroup
          ref={fieldRef}
          {...(force ? { "data-force": force } : {})}
          className={cn(
            "relative flex w-full min-w-0 items-center rounded-pill material-field transition-control cursor-text",
            KAESO_COMBOBOX_VARIANTS.size[size].classes,
            "pr-4",
            (invalid || error) && "ring-invalid",
            !(label || description || error) && className,
          )}
        >
          {inputEl}
          <BaseCombobox.Trigger
            aria-label={t("combobox.showOptions", "Show options")}
            className="inline-flex size-28 shrink-0 items-center justify-center rounded-pill text-fg-tertiary transition-control hover:bg-fill-1 hover:text-fg"
          >
            <Icon icon={icons.chevronDown} size="sm" />
          </BaseCombobox.Trigger>
        </BaseCombobox.InputGroup>
      )}
      <BaseCombobox.Portal container={usePortalContainer()}>
        <BaseCombobox.Positioner
          anchor={fieldRef}
          positionMethod="fixed"
          collisionBoundary={pageBoundary()}
          align="start"
          sideOffset={6}
          collisionPadding={8}
          className="z-50 outline-none"
        >
          <BaseCombobox.Popup className={cn(MENU_POPUP, "relative min-w-anchor overflow-hidden")} style={anchorWidthStyle(fieldWidth)}>
            {loading && (
              <span aria-hidden className="absolute inset-x-16 top-0 h-2 overflow-hidden rounded-pill">
                <span className="block h-full w-1/3 animate-indeterminate rounded-pill bg-fg-tertiary" />
              </span>
            )}
            <BaseCombobox.Status className="sr-only">{loading ? t("combobox.loadingResults", "Loading results") : ""}</BaseCombobox.Status>
            <BaseCombobox.Empty className="flex h-34 items-center px-10 text-body text-fg-secondary empty:hidden">
              {loading ? t("combobox.loading", "Loading…") : (emptyText ?? t("combobox.noResults", "No results"))}
            </BaseCombobox.Empty>
            <BaseCombobox.List className="-m-6 flex min-h-0 flex-col overflow-y-auto overscroll-contain p-6 outline-none scrollbar-quiet">
              {(o: ComboboxOption | CreateItem) => (
                <BaseCombobox.Item key={o.value} value={o} {...(o.disabled ? { disabled: true } : {})} className={MENU_ITEM}>
                  {"__create" in o ? (
                    <>
                      <Icon icon={icons.add} size="sm" className={MENU_ICON} />
                      <span className="min-w-0 flex-1 truncate">
                        {createLabel ? createLabel(o.label) : t("combobox.add", "Add “{label}”", { label: o.label })}
                      </span>
                    </>
                  ) : (
                    <>
                      {o.leading}
                      {o.icon && <Icon icon={o.icon} size="sm" className={MENU_ICON} />}
                      <span className={cn("flex min-w-0 flex-1 flex-col", o.description && "gap-2 py-8")}>
                        <span className="truncate">
                          <Highlight text={o.label} query={query} />
                        </span>
                        {o.description && <span className="text-small font-regular text-fg-secondary">{o.description}</span>}
                      </span>
                      <BaseCombobox.ItemIndicator
                        keepMounted
                        className="flex size-16 shrink-0 items-center justify-center text-fg opacity-0 data-selected:opacity-100"
                      >
                        <Icon icon={icons.check} size="sm" />
                      </BaseCombobox.ItemIndicator>
                    </>
                  )}
                </BaseCombobox.Item>
              )}
            </BaseCombobox.List>
            {hidden > 0 && (
              <div className="flex h-28 shrink-0 items-center px-10 text-caption font-medium text-fg-tertiary">
                {t("combobox.more", "{count, plural, one {# more; type to narrow it down} other {# more; type to narrow them down}}", { count: hidden })}
              </div>
            )}
          </BaseCombobox.Popup>
        </BaseCombobox.Positioner>
      </BaseCombobox.Portal>
    </BaseCombobox.Root>
  );
  if (field) return control;
  return withField(
    {
      label,
      description,
      error,
      optional,
      required,
      layout,
      id: fc.wrapId ?? id,
      disabled,
      className,
    },
    control,
  );
});
Combobox.displayName = "Combobox";

export interface AutocompleteProps extends FieldWrapProps {
  suggestions: string[];
  value?: string | undefined;
  defaultValue?: string | undefined;
  onValueChange?: ((value: string) => void) | undefined;
  placeholder?: string | undefined;
  size?: InputSize | undefined;
  disabled?: boolean | undefined;
  id?: string | undefined;
  name?: string | undefined;
  leadingIcon?: IconData | undefined;
  "aria-label"?: string | undefined;
  className?: string | undefined;
  "data-force"?: string | undefined;
}

export const Autocomplete = forwardRef<HTMLInputElement, AutocompleteProps>(function Autocomplete(props, ref) {
  const {
    suggestions,
    value,
    defaultValue,
    onValueChange,
    placeholder,
    size = "md",
    disabled,
    id,
    name,
    leadingIcon,
    label,
    description,
    error,
    optional,
    required,
    layout,
    className,
    "aria-label": ariaLabel,
    "data-force": force,
  } = props;
  const field = useField();
  const fc = useFieldControl(props);
  const [query, setQuery] = useState(defaultValue ?? "");
  const fieldRef = useRef<HTMLDivElement | null>(null);
  const fieldWidth = useLayoutWidth(fieldRef);
  const current = value ?? query;
  const control = (
    <BaseAutocomplete.Root
      items={suggestions}
      {...(value !== undefined ? { value } : {})}
      {...(defaultValue !== undefined ? { defaultValue } : {})}
      onValueChange={(v: string) => {
        setQuery(v);
        onValueChange?.(v);
      }}
      {...(disabled ? { disabled } : {})}
      {...(name ? { name } : {})}
    >
      <BaseAutocomplete.InputGroup
        ref={fieldRef}
        {...(force ? { "data-force": force } : {})}
        className={cn(
          "relative flex w-full min-w-0 items-center rounded-pill material-field transition-control cursor-text",
          KAESO_COMBOBOX_VARIANTS.size[size].classes,
          leadingIcon && "pl-14",
          props.error && "ring-invalid",
          !(label || description || error) && className,
        )}
      >
        {leadingIcon && <Icon icon={leadingIcon} size="sm" className="text-fg-tertiary" />}
        <BaseAutocomplete.Input
          ref={ref}
          placeholder={placeholder}
          {...(id ? { id } : {})}
          {...fc.controlProps}
          {...(ariaLabel ? { "aria-label": ariaLabel } : {})}
          onKeyDown={keepValueOnEscape}
          className="min-w-0 flex-1 bg-transparent text-fg outline-none placeholder:text-fg-tertiary"
        />
      </BaseAutocomplete.InputGroup>
      <BaseAutocomplete.Portal container={usePortalContainer()}>
        <BaseAutocomplete.Positioner
          anchor={fieldRef}
          positionMethod="fixed"
          collisionBoundary={pageBoundary()}
          align="start"
          sideOffset={6}
          collisionPadding={8}
          className="z-50 outline-none"
        >
          <BaseAutocomplete.Popup className={cn(MENU_POPUP, "min-w-anchor overflow-hidden data-empty:hidden")} style={anchorWidthStyle(fieldWidth)}>
            <BaseAutocomplete.List className="-m-6 flex min-h-0 flex-col overflow-y-auto overscroll-contain p-6 outline-none scrollbar-quiet">
              {(s: string) => (
                <BaseAutocomplete.Item key={s} value={s} className={MENU_ITEM}>
                  <span className="truncate">
                    <Highlight text={s} query={current} />
                  </span>
                </BaseAutocomplete.Item>
              )}
            </BaseAutocomplete.List>
          </BaseAutocomplete.Popup>
        </BaseAutocomplete.Positioner>
      </BaseAutocomplete.Portal>
    </BaseAutocomplete.Root>
  );
  if (field) return control;
  return withField(
    {
      label,
      description,
      error,
      optional,
      required,
      layout,
      id: fc.wrapId ?? id,
      disabled,
      className,
    },
    control,
  );
});
Autocomplete.displayName = "Autocomplete";
