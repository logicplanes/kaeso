import { forwardRef, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { Icon, type IconData } from "../icon/icon.js";
import { icons } from "../../icons/vocabulary.js";
import { Menu } from "../menu/menu.js";
import { CommandList, type CommandListGroup } from "../command-list/command-list.js";
import { PickerPopover, type PickerPopoverProps } from "../command-list/picker-popover.js";
import { useLocale, useUiText } from "../../i18n/context.js";

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
  group?: string | undefined;
}

const SEARCH_ABOVE = 8;
const LIST_ABOVE = 50;

function inRuns(values: FilterChipChoice[], item: (v: FilterChipChoice) => ReactNode): ReactNode {
  if (!values.some((v) => v.group)) return values.map(item);
  const runs: { group: string | undefined; values: FilterChipChoice[] }[] = [];
  for (const v of values) {
    const last = runs[runs.length - 1];
    if (last && last.group === v.group) last.values.push(v);
    else runs.push({ group: v.group, values: [v] });
  }
  return runs.map((r, i) => (
    <Menu.Group key={`${r.group ?? ""}:${i}`} label={r.group}>
      {r.values.map(item)}
    </Menu.Group>
  ));
}

function choiceGroups(values: FilterChipChoice[]): CommandListGroup[] {
  const groups = new Map<string, CommandListGroup>();
  for (const v of values) {
    const key = v.group ?? "";
    const group = groups.get(key) ?? { id: key || "values", ...(key ? { label: key } : {}), items: [] };
    group.items.push({
      value: v.value,
      label: textOf(v),
      keywords: [v.value],
      ...(v.leading || v.icon ? { leading: v.leading ?? (v.icon ? <Icon icon={v.icon} size="sm" className="text-fg-secondary" /> : undefined) } : {}),
      ...(typeof v.label === "string" ? {} : { render: () => <span className="min-w-0 flex-1 truncate">{v.label}</span> }),
    });
    groups.set(key, group);
  }
  return [...groups.values()];
}

export interface FilterChipProps {
  property: ReactNode;
  propertyIcon?: IconData | undefined;
  operator: string;
  operators?: string[] | undefined;
  onOperatorChange?: ((operator: string) => void) | undefined;
  value: ReactNode;
  values?: FilterChipChoice[] | undefined;
  groups?: CommandListGroup[] | undefined;
  searchable?: boolean | undefined;
  width?: PickerPopoverProps["width"] | undefined;
  selected?: string[] | undefined;
  multiple?: boolean | undefined;
  onSelectedChange?: ((selected: string[]) => void) | undefined;
  onRemove?: (() => void) | undefined;
  defaultValuesOpen?: boolean | undefined;
  onValuesOpenChange?: ((open: boolean) => void) | undefined;
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
    groups: givenGroups,
    searchable: forceSearch,
    width,
    selected = [],
    multiple,
    onSelectedChange,
    onRemove,
    defaultValuesOpen,
    onValuesOpenChange,
    className,
    "data-force": force,
  },
  ref,
) {
  const t = useUiText();
  const locale = useLocale();
  const choices = givenGroups ? givenGroups.reduce((n, g) => n + g.items.length, 0) : (values?.length ?? 0);
  const [open, setOwnOpen] = useState(!!defaultValuesOpen && choices > 0);
  const setOpen = (o: boolean) => {
    setOwnOpen(o);
    onValuesOpenChange?.(o);
  };
  const [query, setQuery] = useState("");
  const search = useRef<HTMLInputElement | null>(null);
  const searchable = !!forceSearch || choices > SEARCH_ABOVE;
  const long = !!givenGroups || choices > LIST_ABOVE;
  const groups = useMemo(() => givenGroups ?? (long && values ? choiceGroups(values) : []), [givenGroups, long, values]);
  const q = query.trim().toLowerCase();
  const shown = searchable && q ? (values ?? []).filter((v) => textOf(v).toLowerCase().includes(q) || v.value.toLowerCase().includes(q)) : (values ?? []);
  useEffect(() => {
    if (!open || !searchable || long) return;
    const timer = setTimeout(() => search.current?.focus(), 0);
    return () => clearTimeout(timer);
  }, [open, searchable, long]);
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
      {long && (givenGroups || values) ? (
        <PickerPopover
          title={typeof property === "string" ? property : t("filterChip.filter", "Filter")}
          open={open}
          onOpenChange={setOpen}
          {...(width ? { width } : {})}
          trigger={
            <button type="button" className={cn(SEG, "divider-l", !onRemove && "pr-12")}>
              {value}
            </button>
          }
        >
          <CommandList
            groups={groups}
            aria-label={typeof property === "string" ? property : t("filterChip.filter", "Filter")}
            value={selected}
            {...(multiple ? { multiple: true } : {})}
            autoFocus
            placeholder={
              typeof property === "string"
                ? t("filterChip.searchNamedValues", "Search {property} values", { property: locale === "en" ? property.toLowerCase() : property })
                : t("filterChip.searchValues", "Search values")
            }
            emptyText={t("filterChip.noMatches", "No values match")}
            onSelect={(v) => {
              if (multiple) onSelectedChange?.(selected.includes(v) ? selected.filter((s) => s !== v) : [...selected, v]);
              else {
                onSelectedChange?.([v]);
                setOpen(false);
              }
            }}
          />
        </PickerPopover>
      ) : (
        <Menu
          open={open}
          onOpenChange={(o) => {
            setOpen(o);
            if (!o) setQuery("");
          }}
        >
          <Menu.Trigger>
            <button type="button" className={cn(SEG, "divider-l", !onRemove && "pr-12")} disabled={choices === 0}>
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
                inRuns(shown, (v) => (
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
                  {inRuns(shown, (v) => (
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
      )}
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
