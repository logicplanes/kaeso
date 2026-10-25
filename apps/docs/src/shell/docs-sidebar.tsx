import { Suspense, lazy, useState } from "react";
import { useLocation } from "@tanstack/react-router";
import { Icon, IconButton, Sidebar, icons, useTheme } from "@logicplanes/kaeso-ui";
import { homePath, type Locale } from "~/i18n/locales";
import { useSiteLocale, useText } from "~/i18n/text";
import type { NavGroup } from "~/markdown/ast";
import type { FooterMenu } from "./footer-menus";
import { openSearch } from "./search-store";

export function SearchButton({ tooltip = true }: { tooltip?: boolean }) {
  const t = useText();
  return <IconButton icon={icons.search} aria-label={t("search.open")} aria-haspopup="dialog" tooltip={tooltip} onClick={openSearch} />;
}

const CHANGELOG: Record<Locale, string> = {
  en: "https://kaeso.app/changelog",
  de: "https://kaeso.app/de/changelog",
  "zh-CN": "https://kaeso.app/zh-CN/changelog",
};

const FooterMenus = lazy(() => import("./footer-menus"));

function FooterButtons({ onOpen }: { onOpen: (menu: FooterMenu) => void }) {
  const t = useText();
  const { resolvedTheme } = useTheme();
  return (
    <>
      <IconButton icon={icons.legal} aria-label={t("legal.label")} aria-haspopup="menu" onClick={() => onOpen("legal")} />
      <IconButton
        icon={resolvedTheme === "dark" ? icons.themeDark : icons.themeLight}
        aria-label={t("theme.label")}
        aria-haspopup="menu"
        onClick={() => onOpen("theme")}
      />
      <IconButton icon={icons.language} aria-label={t("language.label")} aria-haspopup="menu" onClick={() => onOpen("language")} />
    </>
  );
}

export function DocsSidebar({ nav }: { nav: NavGroup[] }) {
  const t = useText();
  const [opening, setOpening] = useState<FooterMenu | null>(null);
  const locale = useSiteLocale();
  const pathname = useLocation({ select: (l) => l.pathname.replace(/\/+$/, "") || "/" });
  return (
    <Sidebar aria-label={t("site.navigation")}>
      <Sidebar.Brand href={homePath(locale)} label={t("site.home")}>
        <SearchButton />
        <Sidebar.CollapseToggle />
      </Sidebar.Brand>
      <Sidebar.Content>
        {nav.map((group, i) => (
          <Sidebar.Group key={group.id} id={group.id} {...(group.label ? { label: group.label } : {})}>
            {group.items.map((item) => (
              <Sidebar.Item key={item.path} href={item.path} active={pathname === item.path} depth={item.depth}>
                {item.label}
              </Sidebar.Item>
            ))}
            {i === 0 && (
              <Sidebar.Item href={CHANGELOG[locale]} trailing={<Icon icon={icons.externalLink} size="xs" />}>
                {t("nav.changelog")}
              </Sidebar.Item>
            )}
          </Sidebar.Group>
        ))}
      </Sidebar.Content>
      <Sidebar.Footer className="gap-8">
        <Suspense fallback={<FooterButtons onOpen={setOpening} />}>
          <FooterMenus opening={opening} />
        </Suspense>
      </Sidebar.Footer>
    </Sidebar>
  );
}
