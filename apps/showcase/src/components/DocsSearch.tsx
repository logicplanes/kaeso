import { useMemo } from "react";
import { navigate } from "astro:transitions/client";
import { CommandPalette, icons, type CommandGroup, type IconName } from "@logicplanes/kaeso-ui";
import registry from "../../../../packages/ui/ai/component-registry.json";
import type { NavSection } from "~/lib/nav";

type Reg = { components: { entry: string; components: { name: string; props: { name: string }[] }[] }[] };

export function DocsSearch({ sections, open, onOpenChange }: { sections: NavSection[]; open: boolean; onOpenChange: (o: boolean) => void }) {
  const groups = useMemo<CommandGroup[]>(() => {
    const props = new Map<string, string[]>();
    for (const c of (registry as Reg).components)
      props.set(
        c.entry,
        c.components.flatMap((x) => [x.name, ...x.props.map((p) => p.name)]),
      );
    const go = (href: string) => () => {
      onOpenChange(false);
      void navigate(href);
    };
    const pages: CommandGroup[] = sections.map((s) => ({
      label: s.title,
      items: s.items.map((it) => {
        const slug = it.href.split("/").pop() ?? "";
        return {
          id: it.href,
          label: it.title,
          icon: it.icon ? icons[it.icon as IconName] : icons.code,
          hint: s.title === "Overview" ? undefined : s.title,
          keywords: props.get(slug) ?? [],
          onSelect: go(it.href),
        };
      }) as CommandGroup["items"],
    }));
    return pages;
  }, [sections, onOpenChange]);
  return (
    <CommandPalette
      open={open}
      onOpenChange={onOpenChange}
      groups={groups}
      placeholder="Search pages, components and props"
      emptyText="Nothing matches. Try a component or a prop name."
    />
  );
}
