import { useEffect, useMemo, useState } from "react";
import { navigate } from "astro:transitions/client";
import { CommandPalette, icons, type CommandGroup, type IconName } from "@logicplanes/kaeso-ui";
import type { NavSection } from "~/lib/nav";
import { matchReason, searchKeywords, type MatchLabels, type SearchIndex } from "~/lib/search";
import { docsText, useDocsLang } from "~/lib/locale";

export function DocsSearch({ sections, open, onOpenChange }: { sections: NavSection[]; open: boolean; onOpenChange: (o: boolean) => void }) {
  const { lang } = useDocsLang();
  const [query, setQuery] = useState("");
  // Cleared as the dialog opens, not as it closes: the closing dialog keeps its results while it fades.
  const [wasOpen, setWasOpen] = useState(open);
  if (open !== wasOpen) {
    setWasOpen(open);
    if (open) setQuery("");
  }
  // Every page carries the sidebar; the index is only needed once someone searches.
  const [index, setIndex] = useState<SearchIndex | null>(null);
  useEffect(() => {
    if (open && !index) void import("~/generated/search.json").then((m) => setIndex(m.default));
  }, [open, index]);
  const t = useMemo(() => (english: string) => docsText(lang, english), [lang]);
  const labels = useMemo<MatchLabels>(
    () => ({
      part: t("Part"),
      prop: t("Prop"),
      description: t("Description"),
      example: t("Example"),
      named: (label, name) =>
        t("{label}: {name}")
          .replace("{label}", () => label)
          .replace("{name}", () => name),
    }),
    [t],
  );
  const keywords = useMemo(() => new Map(Object.entries(index ?? {}).map(([slug, entry]) => [slug, searchKeywords(entry, t)])), [index, t]);
  const groups = useMemo<CommandGroup[]>(() => {
    const go = (href: string) => () => {
      onOpenChange(false);
      void navigate(href);
    };
    return sections.map((s) => {
      const section = docsText(lang, s.title);
      return {
        label: section,
        items: s.items.map((it) => {
          const slug = it.href.split("/").pop() ?? "";
          const overview = s.title === "Overview";
          const label = overview ? docsText(lang, it.title) : it.title;
          const entry = overview ? undefined : index?.[slug];
          const reason = entry && query.trim() ? matchReason(label, entry, query, t, labels) : null;
          return {
            id: it.href,
            label,
            icon: it.icon ? icons[it.icon as IconName] : icons.code,
            hint: reason ?? (overview ? undefined : section),
            // The section's name stays searchable when a reason takes the hint's place.
            keywords: overview ? [] : [section, ...(keywords.get(slug) ?? [])],
            onSelect: go(it.href),
          };
        }) as CommandGroup["items"],
      };
    });
  }, [sections, onOpenChange, lang, query, t, labels, keywords, index]);
  return (
    <CommandPalette
      open={open}
      onOpenChange={onOpenChange}
      groups={groups}
      query={query}
      onQueryChange={setQuery}
      placeholder={docsText(lang, "Search pages, components, props and examples")}
      emptyText={docsText(lang, "Nothing matches. Try a component or a prop name.")}
    />
  );
}
