import { useEffect, useState, type ComponentType } from "react";
import { Outlet } from "@tanstack/react-router";
import { AppFrame, Sidebar, TooltipProvider } from "@logicplanes/kaeso-ui";
import { homePath, type Locale } from "~/i18n/locales";
import { useText } from "~/i18n/text";
import type { NavGroup, SearchEntry } from "~/markdown/ast";
import { loadSearchIndex } from "~/site/data";
import { DocsSidebar } from "./docs-sidebar";
import { listenForSearchKey, setSearchOpen, useSearchOpen } from "./search-store";
import { toast } from "./toast";

type SearchComponent = ComponentType<{ index: SearchEntry[] }>;

interface LoadedSearch {
  Search: SearchComponent;
  index: SearchEntry[];
}

let searchModule: Promise<SearchComponent> | null = null;

function loadSearchComponent(): Promise<SearchComponent> {
  searchModule ??= import("./search").then(
    (m) => m.default,
    (error: unknown) => {
      searchModule = null;
      throw error;
    },
  );
  return searchModule;
}

async function loadSearch(locale: Locale): Promise<LoadedSearch> {
  const [Search, index] = await Promise.all([loadSearchComponent(), loadSearchIndex(locale)]);
  return { Search, index };
}

const SEARCH_PRELOAD_DELAY = 1500;

export function DocsShell({ locale, nav }: { locale: Locale; nav: NavGroup[] }) {
  const t = useText();
  const searchOpen = useSearchOpen();
  const [search, setSearch] = useState<LoadedSearch | null>(null);
  useEffect(listenForSearchKey, []);
  useEffect(() => {
    const id = setTimeout(() => void loadSearch(locale).catch(() => undefined), SEARCH_PRELOAD_DELAY);
    return () => clearTimeout(id);
  }, [locale]);
  useEffect(() => {
    if (!searchOpen || search) return;
    let current = true;
    loadSearch(locale).then(
      (loaded) => {
        if (current) setSearch(loaded);
      },
      () => {
        setSearchOpen(false);
        toast.error(t("search.loadFailed"), { id: "search-load" });
      },
    );
    return () => {
      current = false;
    };
  }, [searchOpen, search, locale, t]);
  return (
    <TooltipProvider>
      <Sidebar.Provider storageKey="kaeso-docs:sidebar">
        <AppFrame sidebar={<DocsSidebar nav={nav} />} brandHref={homePath(locale)} brandLabel={t("site.home")}>
          <Outlet />
        </AppFrame>
      </Sidebar.Provider>
      {search && <search.Search index={search.index} />}
    </TooltipProvider>
  );
}
