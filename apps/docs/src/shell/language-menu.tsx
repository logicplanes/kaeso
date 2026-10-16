import { Button, Menu, icons } from "@logicplanes/kaeso-ui";
import { LANGUAGE_NAMES, LOCALES, isLocale } from "~/i18n/locales";
import { useSiteLocale, useText } from "~/i18n/text";
import { useSwitchLocale } from "./use-switch-locale";

export function LanguageMenu() {
  const t = useText();
  const locale = useSiteLocale();
  const switchTo = useSwitchLocale();
  return (
    <Menu>
      <Menu.Trigger>
        <Button
          variant="ghost"
          size="sm"
          trailingIcon={icons.chevronDown}
          aria-label={t("language.current").replace("{language}", LANGUAGE_NAMES[locale])}
          className="mr-auto"
        >
          {LANGUAGE_NAMES[locale]}
        </Button>
      </Menu.Trigger>
      <Menu.Content align="start" className="min-w-180">
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
}
