import { forwardRef, useEffect, useRef } from "react";
import { IconButton, Menu, ThemeMenu, icons } from "@logicplanes/kaeso-ui";
import { LANGUAGE_NAMES, LOCALES, isLocale } from "~/i18n/locales";
import { useSiteLocale, useText } from "~/i18n/text";
import { useSwitchLocale } from "./use-switch-locale";

const PRIVACY = "https://kaeso.app/datenschutz";
const IMPRINT = "https://kaeso.app/impressum";

export type FooterMenu = "legal" | "theme" | "language";

export default function FooterMenus({ opening }: { opening: FooterMenu | null }) {
  const legal = useRef<HTMLButtonElement>(null);
  const theme = useRef<HTMLButtonElement>(null);
  const language = useRef<HTMLButtonElement>(null);
  const clickedWhileLoading = useRef(opening);
  useEffect(() => {
    const trigger = clickedWhileLoading.current && { legal, theme, language }[clickedWhileLoading.current].current;
    trigger?.focus();
    trigger?.click();
  }, []);
  return (
    <>
      <LegalMenu ref={legal} />
      <ThemeMenu ref={theme} side="top" align="start" />
      <LanguageMenu ref={language} />
    </>
  );
}

const LegalMenu = forwardRef<HTMLButtonElement>(function LegalMenu(_, ref) {
  const t = useText();
  return (
    <Menu>
      <Menu.Trigger>
        <IconButton ref={ref} icon={icons.legal} aria-label={t("legal.label")} />
      </Menu.Trigger>
      <Menu.Content side="top" align="start" className="min-w-180">
        <Menu.LinkItem href={PRIVACY}>{t("legal.privacy")}</Menu.LinkItem>
        <Menu.LinkItem href={IMPRINT}>{t("legal.imprint")}</Menu.LinkItem>
      </Menu.Content>
    </Menu>
  );
});

const LanguageMenu = forwardRef<HTMLButtonElement>(function LanguageMenu(_, ref) {
  const t = useText();
  const locale = useSiteLocale();
  const switchTo = useSwitchLocale();
  return (
    <Menu>
      <Menu.Trigger>
        <IconButton ref={ref} icon={icons.language} aria-label={t("language.label")} />
      </Menu.Trigger>
      <Menu.Content side="top" align="start" className="min-w-180">
        <Menu.RadioGroup
          value={locale}
          onValueChange={(value) => {
            if (isLocale(value) && value !== locale) switchTo(value);
          }}
        >
          {LOCALES.map((l) => (
            <Menu.RadioItem key={l} value={l}>
              <span lang={l}>{LANGUAGE_NAMES[l]}</span>
            </Menu.RadioItem>
          ))}
        </Menu.RadioGroup>
      </Menu.Content>
    </Menu>
  );
});
