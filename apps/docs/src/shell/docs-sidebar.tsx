import { useLocation } from "@tanstack/react-router";
import { Icon, IconButton, Link, Sidebar, Text, ThemeSelect, icons } from "@logicplanes/kaeso-ui";
import { homePath, type Locale } from "~/i18n/locales";
import { useSiteLocale, useText } from "~/i18n/text";
import type { NavGroup } from "~/markdown/ast";
import { LanguageSelect } from "./language-select";
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

const PRIVACY = "https://kaeso.app/datenschutz";
const IMPRINT = "https://kaeso.app/impressum";

export function DocsSidebar({ nav }: { nav: NavGroup[] }) {
  const t = useText();
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
      <Sidebar.Footer>
        <div className="flex w-full flex-col gap-8">
          <div className="grid w-full grid-cols-2 gap-4">
            <LanguageSelect />
            <ThemeSelect fullWidth />
          </div>
          <Text as="div" variant="small" className="mx-12 flex gap-12">
            <Link href={PRIVACY} variant="quiet" external={false}>
              {t("nav.privacy")}
            </Link>
            <Link href={IMPRINT} variant="quiet" external={false}>
              {t("nav.imprint")}
            </Link>
          </Text>
        </div>
      </Sidebar.Footer>
    </Sidebar>
  );
}
