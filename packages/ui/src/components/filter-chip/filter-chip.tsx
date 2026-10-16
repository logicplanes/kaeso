import { forwardRef, useEffect, useRef, useState, type ReactNode } from "react";
import { cn } from "../../utils/cn";
import { Icon, type IconData } from "../icon/icon";
import { icons } from "../../icons/vocabulary";
import { Menu } from "../menu/menu";
import { useLocale, useUiText } from "../../i18n/context";

export const KAESO_FILTER_CHIP_VARIANTS = {
  size: {
    sm: { classes: "h-32 text-body", description: "32px: toolbars (default)" },
  },
} as const;
export const KAESO_FILTER_CHIP_DEFAULT_VARIANTS = { size: "sm" } as const;

export interface FilterChipChoice {
  value: string;
  label: ReactNode;
  icon?: IconData | undefined;
  leading?: ReactNode | undefined;
  textValue?: string | undefined;
}

const SEARCH_ABOVE = 8;

export interface FilterChipProps {
  property: ReactNode;
  propertyIcon?: IconData | undefined;
  operator: string;
  operators?: string[] | undefined;
  onOperatorChange?: ((operator: string) => void) | undefined;
  value: ReactNode;
  values?: FilterChipChoice[] | undefined;
  selected?: string[] | undefined;
  multiple?: boolean | undefined;
  onSelectedChange?: ((selected: string[]) => void) | undefined;
  onRemove?: (() => void) | undefined;
  defaultValuesOpen?: boolean | undefined;
  className?: string | undefined;
  "data-force"?: string | undefined;
}

const textOf = (v: FilterChipChoice) => v.textValue ?? (typeof v.label === "string" || typeof v.label === "number" ? String(v.label) : v.value);

const SEG =
  "relative inline-flex h-full cursor-pointer items-center gap-6 px-10 font-medium transition-control tap-target hover:bg-fill-1 active:bg-fill-2 open:bg-fill-1 focus-ring-segment";

export const FilterChip = forwardRef<HTMLDivElement, FilterChipProps>(function FilterChip(
  {
    property,
    propertyIcon,
    operator,
    operators,
    onOperatorChange,
    value,
    values,
    selected = [],
    multiple,
    onSelectedChange,
    onRemove,
    defaultValuesOpen,
    className,
    "data-force": force,
  },
  ref,
) {
  const t = useUiText();
  const locale = useLocale();
  const [open, setOpen] = useState(!!defaultValuesOpen && !!values?.length);
  const [query, setQuery] = useState("");
  const search = useRef<HTMLInputElement | null>(null);
  const searchable = (values?.length ?? 0) > SEARCH_ABOVE;
  const q = query.trim().toLowerCase();
  const shown = searchable && q ? (values ?? []).filter((v) => textOf(v).toLowerCase().includes(q) || v.value.toLowerCase().includes(q)) : (values ?? []);
  useEffect(() => {
    if (!open || !searchable) return;
    const timer = setTimeout(() => search.current?.focus(), 0);
    return () => clearTimeout(timer);
  }, [open, searchable]);
  return (
    <div
      ref={ref}
      role="group"
      aria-label={typeof property === "string" ? t("filterChip.filterNamed", "Filter: {property}", { property }) : t("filterChip.filter", "Filter")}
      {...(force ? { "data-force": force } : {})}
      className={cn(
        "group/fc inline-flex shrink-0 items-stretch overflow-hidden rounded-pill bg-raised text-fg ring-over",
        KAESO_FILTER_CHIP_VARIANTS.size.sm.classes,
        className,
      )}
    >
      <span className={cn(SEG, "cursor-default pl-12 text-fg-secondary hover:bg-transparent active:bg-transparent")}>
        {propertyIcon && <Icon icon={propertyIcon} size="sm" />}
        {property}
      </span>
      <Menu>
        <Menu.Trigger>
          <button type="button" className={cn(SEG, "text-fg-secondary divider-l")} disabled={!operators?.length}>
            {operator}
          </button>
        </Menu.Trigger>
        {operators && (
          <Menu.Content>
            <Menu.RadioGroup value={operator} onValueChange={(v) => onOperatorChange?.(v)}>
              {operators.map((o) => (
                <Menu.RadioItem key={o} value={o}>
                  {o}
                </Menu.RadioItem>
              ))}
            </Menu.RadioGroup>
          </Menu.Content>
        )}
      </Menu>
      <Menu
        open={open}
        onOpenChange={(o) => {
          setOpen(o);
          if (!o) setQuery("");
        }}
      >
        <Menu.Trigger>
          <button type="button" className={cn(SEG, "divider-l", !onRemove && "pr-12")} disabled={!values?.length}>
            {value}
          </button>
        </Menu.Trigger>
        {values && (
          <Menu.Content
            {...(searchable ? { className: "w-280" } : {})}
            {...(searchable
              ? {
                  header: (
                    <input
                      ref={search}
                      type="search"
                      aria-label={
                        typeof property === "string"
                          ? t("filterChip.searchNamedValues", "Search {property} values", { property: locale === "en" ? property.toLowerCase() : property })
                          : t("filterChip.searchValues", "Search values")
                      }
                      placeholder={t("filterChip.search", "Search")}
                      autoComplete="off"
                      value={query}
                      onChange={(e) => setQuery(e.target.value)}
                      onKeyDown={(e) => {
                        if (!["ArrowDown", "ArrowUp", "Escape", "Tab", "Enter"].includes(e.key)) e.stopPropagation();
                      }}
                      className="h-28 w-full bg-transparent text-body text-fg outline-none placeholder:text-fg-tertiary coarse:h-36"
                    />
                  ),
                  ...(shown.length === 0 ? { footer: t("filterChip.noMatches", "No values match") } : {}),
                }
              : {})}
          >
            {multiple ? (
              shown.map((v) => (
                <Menu.CheckboxItem
                  key={v.value}
                  checked={selected.includes(v.value)}
                  onCheckedChange={(c) => onSelectedChange?.(c ? [...selected, v.value] : selected.filter((s) => s !== v.value))}
                  {...(v.icon ? { icon: v.icon } : {})}
                >
                  <span className="inline-flex items-center gap-8">
                    {v.leading}
                    {v.label}
                  </span>
                </Menu.CheckboxItem>
              ))
            ) : (
              <Menu.RadioGroup value={selected[0] ?? ""} onValueChange={(s) => onSelectedChange?.([s])}>
                {shown.map((v) => (
                  <Menu.RadioItem key={v.value} value={v.value} closeOnClick {...(v.icon ? { icon: v.icon } : {})}>
                    <span className="inline-flex items-center gap-8">
                      {v.leading}
                      {v.label}
                    </span>
                  </Menu.RadioItem>
                ))}
              </Menu.RadioGroup>
            )}
          </Menu.Content>
        )}
      </Menu>
      {onRemove && (
        <span className="flex items-center pr-4 divider-l pl-4">
          <button
            type="button"
            aria-label={
              typeof property === "string" ? t("filterChip.removeNamed", "Remove the {property} filter", { property }) : t("filterChip.remove", "Remove filter")
            }
            onClick={onRemove}
            className="relative inline-flex size-24 cursor-pointer items-center justify-center rounded-pill text-fg-tertiary transition-control hover:bg-fill-2 hover:text-fg focus-ring tap-target"
          >
            <Icon icon={icons.close} size="xs" />
          </button>
        </span>
      )}
    </div>
  );
});
FilterChip.displayName = "FilterChip";
