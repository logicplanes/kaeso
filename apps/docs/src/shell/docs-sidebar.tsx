import { useLocation } from "@tanstack/react-router";
import { IconButton, Sidebar, ThemeMenu, icons } from "@logicplanes/kaeso-ui";
import { homePath } from "~/i18n/locales";
import { useSiteLocale, useText } from "~/i18n/text";
import type { NavGroup } from "~/markdown/ast";
import { LanguageMenu } from "./language-menu";
import { openSearch } from "./search-store";

export function DocsSidebar({ nav }: { nav: NavGroup[] }) {
  const t = useText();
  const locale = useSiteLocale();
  const pathname = useLocation({ select: (l) => l.pathname.replace(/\/+$/, "") || "/" });
  return (
    <Sidebar aria-label={t("site.navigation")}>
      <Sidebar.Brand href={homePath(locale)} label={t("site.home")}>
        <IconButton icon={icons.search} aria-label={t("search.open")} onClick={openSearch} />
        <Sidebar.CollapseToggle />
      </Sidebar.Brand>
      <Sidebar.Content>
        {nav.map((group) => (
          <Sidebar.Group key={group.id} id={group.id} {...(group.label ? { label: group.label } : {})}>
            {group.items.map((item) => (
              <Sidebar.Item key={item.path} href={item.path} active={pathname === item.path} depth={item.depth}>
                {item.label}
              </Sidebar.Item>
            ))}
          </Sidebar.Group>
        ))}
      </Sidebar.Content>
      <Sidebar.Footer>
        <LanguageMenu />
        <ThemeMenu />
      </Sidebar.Footer>
    </Sidebar>
  );
}
