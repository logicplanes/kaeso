import { Select } from "@logicplanes/kaeso-ui";
import { LANGUAGE_NAMES, LOCALES, isLocale } from "~/i18n/locales";
import { useSiteLocale, useText } from "~/i18n/text";
import { useSwitchLocale } from "./use-switch-locale";

export function LanguageSelect() {
  const t = useText();
  const locale = useSiteLocale();
  const switchTo = useSwitchLocale();
  return (
    <Select
      aria-label={t("language.label")}
      size="sm"
      appearance="ghost"
      fullWidth
      value={locale}
      onValueChange={(value) => {
        if (isLocale(value) && value !== locale) switchTo(value);
      }}
      options={LOCALES.map((l) => ({ value: l, label: <span lang={l}>{LANGUAGE_NAMES[l]}</span>, textValue: LANGUAGE_NAMES[l] }))}
    />
  );
}
