import { useEffect, useEffectEvent, useState, type ReactNode, type RefObject } from "react";
import { useQuery } from "@tanstack/react-query";
import { Button, DateRangePicker, FilterChip, type FilterChipChoice, type FilterChipProps, Input, Menu, Select, icons } from "@logicplanes/kaeso-ui";
import type { LogRange } from "~/api/queries/logs";
import { allKeysQuery } from "~/api/queries/keys";
import { projectMembersQuery, serviceAccountsQuery } from "~/api/queries/members";
import { useProjectRights } from "~/auth/rights";
import { dayKey } from "~/format/zone";
import { pickedDay, pickerDate } from "~/format/picked-day";
import { t } from "~/i18n";
import { keyLabel } from "../keys/party";
import { scrollParentOf } from "./live";

export const ranges = (): { value: LogRange; label: string }[] => [
  { value: "1h", label: t("logs.range.hour", "Last hour") },
  { value: "24h", label: t("logs.range.day", "Last 24 hours") },
  { value: "7d", label: t("logs.range.week", "Last 7 days") },
  { value: "30d", label: t("logs.range.month", "Last 30 days") },
];

export interface RangeSearch {
  range?: LogRange | undefined;
  from?: string | undefined;
  to?: string | undefined;
}

export function RangeControls({ value, onChange }: { value: RangeSearch; onChange: (next: Required<RangeSearch>) => void }) {
  return (
    <>
      <div className="shrink-0">
        <Select
          aria-label={t("logs.timeRange", "Time range")}
          size="sm"
          icon={icons.calendar}
          value={value.from ? "custom" : (value.range ?? "24h")}
          onValueChange={(v) =>
            v === "custom"
              ? onChange({ from: dayKey(new Date()), to: dayKey(new Date()), range: undefined })
              : onChange({ range: ranges().find((r) => r.value === v && v !== "24h")?.value, from: undefined, to: undefined })
          }
          options={[...ranges(), { value: "custom", label: t("logs.range.custom", "Custom range") }]}
        />
      </div>
      {value.from && value.to && (
        <div className="w-240 shrink-0">
          <DateRangePicker
            aria-label={t("logs.days", "Days")}
            size="sm"
            value={{ from: pickerDate(value.from), to: pickerDate(value.to) }}
            onValueChange={(r) => r?.from && onChange({ from: pickedDay(r.from), to: pickedDay(r.to ?? r.from), range: undefined })}
          />
        </div>
      )}
    </>
  );
}

export interface FilterDef {
  id: string;
  label: string;
  value: string | undefined;
  flag?: boolean;
  valueLabel?: (value: string) => ReactNode;
  values?: FilterChipChoice[];
  groups?: FilterChipProps["groups"];
  width?: FilterChipProps["width"];
  searchable?: boolean;
}

export function FilterBar({
  filters,
  onChange,
  compact,
}: {
  filters: FilterDef[];
  onChange: (id: string, value: string | undefined) => void;
  compact: boolean;
}) {
  const [adding, setAdding] = useState<string | null>(null);
  const [added, setAdded] = useState<string[]>([]);
  const present = filters.filter((f) => f.value !== undefined || adding === f.id);
  const active = [...present.filter((f) => !added.includes(f.id)), ...added.flatMap((id) => present.filter((f) => f.id === id))];
  const add = (f: FilterDef) => {
    setAdded((a) => [...a.filter((x) => x !== f.id), f.id]);
    if (f.flag) onChange(f.id, "true");
    else setAdding(f.id);
  };
  return (
    <>
      {active.map((f) => {
        const id = f.id;
        if (f.flag)
          return (
            <FilterChip
              key={id}
              property={f.label}
              operator={t("logs.filter.is", "is")}
              value={t("logs.filter.yes", "yes")}
              onRemove={() => onChange(id, undefined)}
            />
          );
        const picked = { current: false };
        return (
          <FilterChip
            key={id}
            property={f.label}
            operator={t("logs.filter.is", "is")}
            value={f.value ? (f.valueLabel?.(f.value) ?? f.values?.find((c) => c.value === f.value)?.label ?? f.value) : t("logs.filter.any", "any")}
            {...(f.groups ? { groups: f.groups, width: f.width } : { values: f.values, searchable: !!f.searchable, width: f.width })}
            selected={f.value ? [f.value] : []}
            defaultValuesOpen={adding === id}
            onValuesOpenChange={(open) => {
              if (!open && !picked.current && adding === id) setAdding(null);
            }}
            onSelectedChange={(v) => {
              picked.current = true;
              setAdding(null);
              onChange(id, v[0]);
            }}
            onRemove={() => {
              setAdding(null);
              onChange(id, undefined);
            }}
          />
        );
      })}
      <Menu>
        <Menu.Trigger>
          {compact ? (
            <Button variant="ghost" icon={icons.add} size="sm" aria-label={t("logs.addFilter", "Add filter")} />
          ) : (
            <Button variant="ghost" size="sm" leadingIcon={icons.add}>
              {t("logs.addFilter", "Add filter")}
            </Button>
          )}
        </Menu.Trigger>
        <Menu.Content>
          {filters
            .filter((f) => !active.includes(f))
            .map((f) => (
              <Menu.Item key={f.id} onClick={() => add(f)}>
                {f.label}
              </Menu.Item>
            ))}
        </Menu.Content>
      </Menu>
    </>
  );
}

export type Density = "default" | "compact";

export const densityRow = (density: Density, onChange: (density: Density) => void) => ({
  label: t("logs.density", "Density"),
  icon: icons.displayOptions,
  value: density,
  onChange: (v: string) => onChange(v as Density),
  options: [
    { value: "default", label: t("logs.density.comfortable", "Comfortable") },
    { value: "compact", label: t("logs.density.compact", "Compact") },
  ],
});

export function LiveButton({ live, onChange }: { live: boolean; onChange: (live: boolean) => void }) {
  return (
    <Button size="sm" variant={live ? "primary" : "secondary"} leadingIcon={icons.refresh} aria-pressed={live} onClick={() => onChange(!live)}>
      {t("logs.live", "Live")}
    </Button>
  );
}

export function IdSearch({ label, placeholder, onOpen }: { label: string; placeholder: string; onOpen: (id: string) => void }) {
  const [value, setValue] = useState("");
  return (
    <div className="hidden w-200 md:block">
      <Input
        type="search"
        size="sm"
        aria-label={label}
        placeholder={placeholder}
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onClear={() => setValue("")}
        onKeyDown={(e) => {
          const id = value.trim();
          if (e.key !== "Enter" || !id) return;
          e.preventDefault();
          onOpen(id);
        }}
      />
    </div>
  );
}

export function usePartyFilters(org: string, project: string) {
  const rights = useProjectRights(org, project);
  const keys = useQuery({ ...allKeysQuery(org, project, { include_archived: true }), enabled: rights.has("keys.all.view") });
  const members = useQuery({ ...projectMembersQuery(org, project), enabled: rights.has("project.members.view") });
  const accounts = useQuery({ ...serviceAccountsQuery(org, project, true), enabled: rights.has("service_accounts.view") });
  const keyName = (id: string) => {
    const key = keys.data?.find((k) => k.id === id);
    return key ? keyLabel(key) : id;
  };
  const keyOptions: FilterChipChoice[] = (keys.data ?? []).map((k) => ({ value: k.id, label: keyLabel(k) }));
  const memberOptions = [
    ...(members.data ?? []).map((m) => ({ value: `user:${m.user.id}`, label: m.user.name })),
    ...(accounts.data ?? []).map((a) => ({ value: `sa:${a.id}`, label: t("logs.filter.serviceAccount", "{name} (service account)", { name: a.name }) })),
  ];
  return { keyName, keyOptions, memberOptions };
}

export function useLoadMoreNearEnd(
  scroller: RefObject<HTMLElement | null>,
  {
    hasNextPage,
    isFetchingNextPage,
    isFetchNextPageError,
    fetchNextPage,
  }: { hasNextPage: boolean; isFetchingNextPage: boolean; isFetchNextPageError: boolean; fetchNextPage: () => unknown },
) {
  const loadMore = useEffectEvent(() => void fetchNextPage());
  useEffect(() => {
    const more = scroller.current?.querySelector("nav");
    if (!more || !hasNextPage || isFetchingNextPage || isFetchNextPageError) return;
    const seen = new IntersectionObserver((entries) => entries.some((e) => e.isIntersecting) && loadMore(), {
      root: scrollParentOf(scroller.current),
      rootMargin: "800px 0px",
    });
    seen.observe(more);
    return () => seen.disconnect();
  }, [scroller, hasNextPage, isFetchingNextPage, isFetchNextPageError]);
}

export function useRowStepping(rows: readonly { id: string }[], open: string | undefined, onOpen: (id: string) => void) {
  const step = useEffectEvent((e: KeyboardEvent) => {
    if (e.metaKey || e.ctrlKey || e.altKey || (e.target as HTMLElement).closest("input, textarea, [contenteditable], [role=combobox]")) return;
    if (e.key !== "j" && e.key !== "k") return;
    const i = rows.findIndex((r) => r.id === open);
    const next = rows[e.key === "j" ? i + 1 : i - 1];
    if (next) {
      e.preventDefault();
      onOpen(next.id);
    }
  });
  useEffect(() => {
    if (!open) return;
    window.addEventListener("keydown", step);
    return () => window.removeEventListener("keydown", step);
  }, [open]);
}
