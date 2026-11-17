import { Suspense, lazy, useState } from "react";
import { useLocation } from "@tanstack/react-router";
import { Icon, Button, Sidebar, Text, ThemeMenu, icons } from "@logicplanes/kaeso-ui";
import { useFormatters } from "@logicplanes/kaeso-ui/i18n";
import { homePath, type Locale } from "~/i18n/locales";
import { useSiteLocale, useText } from "~/i18n/text";
import type { NavGroup } from "~/markdown/ast";
import { LanguageButton } from "./language-button";
import { openSearch } from "./search-store";

export function SearchButton({ tooltip = true }: { tooltip?: boolean }) {
  const t = useText();
  return <Button variant="ghost" size="sm" icon={icons.search} aria-label={t("search.open")} aria-haspopup="dialog" tooltip={tooltip} onClick={openSearch} />;
}

const CHANGELOG: Record<Locale, string> = {
  en: "https://kaeso.app/changelog",
  de: "https://kaeso.app/de/changelog",
  "zh-CN": "https://kaeso.app/zh-CN/changelog",
};

const REPOSITORY = "https://github.com/logicplanes/kaeso";
const STARS_SHOWN_FROM = 100;

const LanguageMenu = lazy(() => import("./footer-menus"));

function StarCount({ stars }: { stars: number | null }) {
  const { formatNumber } = useFormatters();
  return (
    <>
      {stars !== null && stars >= STARS_SHOWN_FROM && (
        <Text variant="caption" tone="tertiary" tabular>
          {formatNumber(stars, { notation: "compact", maximumFractionDigits: 1 })}
        </Text>
      )}
      <Icon icon={icons.externalLink} size="xs" />
    </>
  );
}

export function DocsSidebar({ nav, stars }: { nav: NavGroup[]; stars: number | null }) {
  const t = useText();
  const [opening, setOpening] = useState(false);
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
              <>
                <Sidebar.Item href={CHANGELOG[locale]} trailing={<Icon icon={icons.externalLink} size="xs" />}>
                  {t("nav.changelog")}
                </Sidebar.Item>
                <Sidebar.Item href={REPOSITORY} trailing={<StarCount stars={stars} />}>
                  {t("nav.star")}
                </Sidebar.Item>
              </>
            )}
          </Sidebar.Group>
        ))}
      </Sidebar.Content>
      <Sidebar.Footer className="justify-between gap-8">
        <ThemeMenu display="segmented" size="sm" />
        <Suspense fallback={<LanguageButton onClick={() => setOpening(true)} />}>
          <LanguageMenu opening={opening} />
        </Suspense>
      </Sidebar.Footer>
    </Sidebar>
  );
}
