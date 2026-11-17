import { useEffect, useRef } from "react";
import { Menu } from "@logicplanes/kaeso-ui";
import { LANGUAGE_NAMES, LOCALES, isLocale } from "~/i18n/locales";
import { useSiteLocale } from "~/i18n/text";
import { LanguageButton } from "./language-button";
import { useSwitchLocale } from "./use-switch-locale";

export default function LanguageMenu({ opening }: { opening: boolean }) {
  const trigger = useRef<HTMLButtonElement>(null);
  const clickedWhileLoading = useRef(opening);
  const locale = useSiteLocale();
  const switchTo = useSwitchLocale();
  useEffect(() => {
    if (!clickedWhileLoading.current) return;
    trigger.current?.focus();
    trigger.current?.click();
  }, []);
  return (
    <Menu>
      <Menu.Trigger>
        <LanguageButton ref={trigger} />
      </Menu.Trigger>
      <Menu.Content side="top" align="end" className="min-w-180">
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
