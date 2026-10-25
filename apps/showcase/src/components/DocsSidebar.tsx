import type * as React from "react";
import { useEffect, useRef, useState } from "react";
import {
  useSidebar,
  IconButton,
  Menu,
  NavSheet,
  PortalContainer,
  Toaster,
  Sidebar,
  ThemeProvider,
  ThemeMenu,
  TooltipProvider,
  icons,
} from "@logicplanes/kaeso-ui";
import type { NavSection } from "~/lib/nav";
import { DocsSearch } from "./DocsSearch";
import { DocsLanguageMenu, DocsLocale, docsText, useDocsLang } from "~/lib/locale";
import { localizedPath, type DocsLang } from "~/lib/languages";

const LEGAL: Record<DocsLang, { privacy: string; imprint: string }> = {
  en: { privacy: "https://kaeso.app/datenschutz", imprint: "https://kaeso.app/impressum" },
  de: { privacy: "https://kaeso.app/de/datenschutz", imprint: "https://kaeso.app/de/impressum" },
  "zh-CN": { privacy: "https://kaeso.app/zh-CN/datenschutz", imprint: "https://kaeso.app/zh-CN/impressum" },
};

const clean = (p: string) => p.replace(/\.html$/, "").replace(/\/$/, "") || "/";

export default function DocsSidebar({ sections, path: initial }: { sections: NavSection[]; path: string }) {
  const [path, setPath] = useState(initial);
  const [navOpen, setNavOpen] = useState(false);
  const [searchOpen, setSearchOpen] = useState(false);
  const opener = useRef<HTMLElement | null>(null);
  useEffect(() => {
    document.querySelectorAll("[data-open-nav]").forEach((b) => b.setAttribute("aria-expanded", String(navOpen)));
  }, [navOpen]);
  useEffect(() => {
    const onLoad = () => {
      setPath(clean(location.pathname));
      setNavOpen(false);
    };
    const onOpen = () => {
      opener.current = document.querySelector<HTMLElement>("[data-open-nav]");
      setNavOpen(true);
    };
    const onSearch = () => setSearchOpen(true);
    document.addEventListener("astro:page-load", onLoad);
    window.addEventListener("kaeso:open-nav", onOpen);
    window.addEventListener("kaeso:open-search", onSearch);
    return () => {
      document.removeEventListener("astro:page-load", onLoad);
      window.removeEventListener("kaeso:open-nav", onOpen);
      window.removeEventListener("kaeso:open-search", onSearch);
    };
  }, []);

  const { lang } = useDocsLang();
  const nav = (
    <Sidebar aria-label={docsText(lang, "Documentation")}>
      <Sidebar.Brand href={localizedPath(lang, "/")} label={docsText(lang, "Kaeso UI home")}>
        <IconButton icon={icons.search} aria-label={docsText(lang, "Search")} shortcut={["mod", "k"]} onClick={() => setSearchOpen(true)} />
        <Sidebar.CollapseToggle />
      </Sidebar.Brand>
      <Sidebar.Content>
        {sections.map((s, i) => (
          <Sidebar.Group key={s.title} label={i === 0 ? undefined : docsText(lang, s.title)} id={s.title.toLowerCase()} defaultOpen>
            {s.items.map((it) => (
              <Sidebar.Item key={it.href} href={it.href} active={path === it.href}>
                {i === 0 ? docsText(lang, it.title) : it.title}
              </Sidebar.Item>
            ))}
          </Sidebar.Group>
        ))}
      </Sidebar.Content>
      <Sidebar.Footer className="gap-8">
        <Menu>
          <Menu.Trigger>
            <IconButton icon={icons.legal} aria-label={docsText(lang, "Legal")} />
          </Menu.Trigger>
          <Menu.Content side="top" align="start" className="min-w-180">
            <Menu.LinkItem href={LEGAL[lang].privacy}>{docsText(lang, "Privacy")}</Menu.LinkItem>
            <Menu.LinkItem href={LEGAL[lang].imprint}>{docsText(lang, "Imprint")}</Menu.LinkItem>
          </Menu.Content>
        </Menu>
        <ThemeMenu side="top" align="start" />
        <DocsLanguageMenu />
      </Sidebar.Footer>
    </Sidebar>
  );

  return (
    <DocsLocale>
      <ThemeProvider>
        <PortalContainer>
          <TooltipProvider>
            <Sidebar.Provider storageKey="docs-sidebar">
              <div className="hidden h-full lg:flex">{nav}</div>
              <DocsSheet navOpen={navOpen} setNavOpen={setNavOpen} returnFocus={opener}>
                {nav}
              </DocsSheet>
              <Toaster />
              <DocsSearch sections={sections} open={searchOpen} onOpenChange={setSearchOpen} />
              <SyncCollapsed />
            </Sidebar.Provider>
          </TooltipProvider>
        </PortalContainer>
      </ThemeProvider>
    </DocsLocale>
  );
}

function SyncCollapsed() {
  const { collapsed, setCollapsed } = useSidebar();
  useEffect(() => {
    const apply = () => {
      if (collapsed) document.documentElement.setAttribute("data-sidebar-collapsed", "");
      else document.documentElement.removeAttribute("data-sidebar-collapsed");
    };
    apply();
    const expand = () => setCollapsed(false);
    document.addEventListener("astro:after-swap", apply);
    window.addEventListener("kaeso:expand-sidebar", expand);
    return () => {
      document.removeEventListener("astro:after-swap", apply);
      window.removeEventListener("kaeso:expand-sidebar", expand);
    };
  }, [collapsed, setCollapsed]);
  return null;
}

function DocsSheet({
  navOpen,
  setNavOpen,
  returnFocus,
  children,
}: {
  navOpen: boolean;
  setNavOpen: (o: boolean) => void;
  returnFocus: React.RefObject<HTMLElement | null>;
  children: React.ReactNode;
}) {
  const { peeking, setPeeking, collapsed } = useSidebar();
  useEffect(() => {
    const peek = () => collapsed && setPeeking(true);
    window.addEventListener("kaeso:peek-sidebar", peek);
    return () => window.removeEventListener("kaeso:peek-sidebar", peek);
  }, [collapsed, setPeeking]);
  return (
    <NavSheet
      open={navOpen || peeking}
      peek={peeking && !navOpen}
      {...(navOpen ? { returnFocus } : {})}
      onOpenChange={(o) => {
        if (!o) {
          setNavOpen(false);
          setPeeking(false);
        }
      }}
    >
      {children}
    </NavSheet>
  );
}
