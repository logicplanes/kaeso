import { forwardRef, useMemo, useState, type ReactNode } from "react";
import { List, SelectionAction, type ListGroup, type ListItemData } from "../../components/list/list.js";
import { Toolbar } from "../../components/toolbar/toolbar.js";
import { SearchInput } from "../../components/input/input.js";
import { Empty } from "../../components/empty/empty.js";
import type { IconData } from "../../components/icon/icon.js";
import { useLocale, useUiText } from "../../i18n/context.js";
import { Button } from "../../components/button/button.js";

export const KAESO_RESOURCE_LIST_VARIANTS = {
  grouping: {
    grouped: { classes: "", description: "Rows under group headers (default)" },
    flat: { classes: "", description: "One list" },
  },
} as const;
export const KAESO_RESOURCE_LIST_DEFAULT_VARIANTS = { grouping: "grouped" } as const;

export interface ResourceListProps {
  groups: ListGroup[];
  "aria-label": string;
  onOpen?: ((item: ListItemData) => void) | undefined;
  bulkActions?: { label: string; icon?: IconData | undefined; onClick: (ids: string[]) => void }[] | undefined;
  toolbar?: ReactNode | undefined;
  emptyTitle?: string | undefined;
  emptyDescription?: string | undefined;
  emptyAction?: ReactNode | undefined;
}

function searchText(i: ListItemData) {
  return `${i.textValue ?? (typeof i.title === "string" ? i.title : "")} ${i.displayId ?? ""}`.toLowerCase();
}

export const ResourceList = forwardRef<HTMLDivElement, ResourceListProps>(function ResourceList(
  { groups, onOpen, bulkActions = [], toolbar, emptyTitle, emptyDescription, emptyAction, "aria-label": ariaLabel },
  ref,
) {
  const t = useUiText();
  const locale = useLocale();
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const q = query.trim().toLowerCase();
  const filtered = useMemo(
    () => groups.map((g) => ({ ...g, items: g.items.filter((i) => !q || searchText(i).includes(q)) })).filter((g) => g.items.length > 0),
    [groups, q],
  );
  const visibleSelection = useMemo(() => {
    const shown = new Set(filtered.flatMap((g) => g.items.map((i) => i.id)));
    return new Set([...selected].filter((id) => shown.has(id)));
  }, [filtered, selected]);
  const shown = filtered.reduce((n, g) => n + g.items.length, 0);
  const noun = locale === "en" && /^[A-Z][a-z]/.test(ariaLabel) ? ariaLabel[0]!.toLowerCase() + ariaLabel.slice(1) : ariaLabel;
  return (
    <div ref={ref} className="flex flex-col">
      <span role="status" className="sr-only">
        {q ? t("resourceList.results", "{count, plural, =0 {No results} one {# result} other {# results}}", { count: shown }) : ""}
      </span>
      <Toolbar aria-label={t("resourceList.filters", "Filters")}>
        {toolbar}
        <Toolbar.Spacer />
        <div className="w-240">
          <SearchInput
            aria-label={t("resourceList.search", "Search")}
            placeholder={t("resourceList.search", "Search")}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onClear={() => setQuery("")}
            shortcut={["/"]}
          />
        </div>
      </Toolbar>
      {filtered.length === 0 ? (
        <Empty
          size="md"
          title={q ? t("resourceList.noMatches", "No matches") : (emptyTitle ?? t("resourceList.noneYet", "No {label} yet", { label: noun }))}
          {...(q ? { description: t("resourceList.tryShorterSearch", "Try a shorter search.") } : emptyDescription ? { description: emptyDescription } : {})}
          {...(q
            ? {
                actions: (
                  <Button size="sm" onClick={() => setQuery("")}>
                    {t("resourceList.clearSearch", "Clear search")}
                  </Button>
                ),
              }
            : emptyAction
              ? { actions: emptyAction }
              : {})}
        />
      ) : (
        <List
          aria-label={ariaLabel}
          groups={filtered}
          selectable={bulkActions.length > 0}
          selected={visibleSelection}
          onSelectedChange={setSelected}
          {...(onOpen ? { onOpen } : {})}
          selectionActions={bulkActions.map((a) => (
            <SelectionAction key={a.label} {...(a.icon ? { icon: a.icon } : {})} onClick={() => a.onClick([...visibleSelection])}>
              {a.label}
            </SelectionAction>
          ))}
        />
      )}
    </div>
  );
});
ResourceList.displayName = "ResourceList";
