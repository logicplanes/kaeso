import { useState, type ReactNode } from "react";
import { List, SelectionAction, type ListGroup, type ListItemData } from "../../components/list/list";
import { Toolbar } from "../../components/toolbar/toolbar";
import { SearchInput } from "../../components/input/input";
import { Empty } from "../../components/empty/empty";
import type { IconData } from "../../components/icon/icon";
import { useUiText } from "../../i18n/context";

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

export function ResourceList({
  groups,
  onOpen,
  bulkActions = [],
  toolbar,
  emptyTitle,
  emptyDescription,
  emptyAction,
  "aria-label": ariaLabel,
}: ResourceListProps) {
  const t = useUiText();
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const q = query.trim().toLowerCase();
  const text = (i: ListItemData) => `${i.textValue ?? (typeof i.title === "string" ? i.title : "")} ${i.displayId ?? ""}`.toLowerCase();
  const filtered = groups.map((g) => ({ ...g, items: g.items.filter((i) => !q || text(i).includes(q)) })).filter((g) => g.items.length > 0);
  return (
    <div className="flex flex-col">
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
          title={q ? t("resourceList.noMatches", "No matches") : (emptyTitle ?? t("resourceList.nothingHereYet", "Nothing here yet"))}
          {...(q ? { description: t("resourceList.tryShorterSearch", "Try a shorter search.") } : emptyDescription ? { description: emptyDescription } : {})}
          {...(!q && emptyAction ? { actions: emptyAction } : {})}
        />
      ) : (
        <List
          aria-label={ariaLabel}
          groups={filtered}
          selectable={bulkActions.length > 0}
          selected={selected}
          onSelectedChange={setSelected}
          {...(onOpen ? { onOpen } : {})}
          selectionActions={bulkActions.map((a) => (
            <SelectionAction key={a.label} {...(a.icon ? { icon: a.icon } : {})} onClick={() => a.onClick([...selected])}>
              {a.label}
            </SelectionAction>
          ))}
        />
      )}
    </div>
  );
}
ResourceList.displayName = "ResourceList";
