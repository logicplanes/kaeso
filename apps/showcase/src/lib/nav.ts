import demos from "~/generated/demos.json";

export interface NavItem {
  title: string;
  href: string;
  icon?: string;
}
export interface NavSection {
  title: string;
  items: NavItem[];
}

export const overview: NavItem[] = [
  { icon: "home", title: "Overview", href: "/" },
  { icon: "download", title: "Installation", href: "/installation" },
  { icon: "themeLight", title: "Theming", href: "/theming" },
  { icon: "serviceAccount", title: "For agents", href: "/for-agents" },
  { icon: "time", title: "Changelog", href: "/changelog" },
];

type Page = { slug: string; title: string; group?: string };
const pages = Object.values(demos as Record<string, Page>);

const components = pages
  .filter((p) => p.group !== "Blocks")
  .sort((a, b) => a.title.localeCompare(b.title))
  .map((p) => ({ title: p.title, href: `/components/${p.slug}` }));

export function sections(): NavSection[] {
  return [
    { title: "Overview", items: overview },
    { title: "Components", items: components },
  ];
}

export const GITHUB = "https://github.com/logicplanes/kaeso";
