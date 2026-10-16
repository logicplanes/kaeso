import { useMemo } from "react";
import { navigate } from "astro:transitions/client";
import { CommandPalette, icons, type CommandGroup, type IconName } from "@logicplanes/kaeso-ui";
import type { NavSection } from "~/lib/nav";
import { registry } from "~/lib/registry";
import { docsText, useDocsLang } from "~/lib/locale";

export function DocsSearch({ sections, open, onOpenChange }: { sections: NavSection[]; open: boolean; onOpenChange: (o: boolean) => void }) {
  const { lang } = useDocsLang();
  const groups = useMemo<CommandGroup[]>(() => {
    const props = new Map<string, string[]>();
    for (const c of registry.components)
      props.set(
        c.entry,
        c.components.flatMap((x) => [x.name, ...x.props.map((p) => p.name)]),
      );
    const go = (href: string) => () => {
      onOpenChange(false);
      void navigate(href);
    };
    const pages: CommandGroup[] = sections.map((s) => ({
      label: docsText(lang, s.title),
      items: s.items.map((it) => {
        const slug = it.href.split("/").pop() ?? "";
        return {
          id: it.href,
          label: s.title === "Overview" ? docsText(lang, it.title) : it.title,
          icon: it.icon ? icons[it.icon as IconName] : icons.code,
          hint: s.title === "Overview" ? undefined : docsText(lang, s.title),
          keywords: props.get(slug) ?? [],
          onSelect: go(it.href),
        };
      }) as CommandGroup["items"],
    }));
    return pages;
  }, [sections, onOpenChange, lang]);
  return (
    <CommandPalette
      open={open}
      onOpenChange={onOpenChange}
      groups={groups}
      placeholder={docsText(lang, "Search pages, components and props")}
      emptyText={docsText(lang, "Nothing matches. Try a component or a prop name.")}
    />
  );
}
