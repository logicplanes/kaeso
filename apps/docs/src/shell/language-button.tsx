import { forwardRef, type ComponentProps } from "react";
import { Button, icons } from "@logicplanes/kaeso-ui";
import { LANGUAGE_NAMES } from "~/i18n/locales";
import { useSiteLocale, useText } from "~/i18n/text";

export const LanguageButton = forwardRef<HTMLButtonElement, Omit<ComponentProps<typeof Button>, "children" | "icon">>(function LanguageButton(props, ref) {
  const t = useText();
  const locale = useSiteLocale();
  const name = LANGUAGE_NAMES[locale];
  return (
    <Button
      ref={ref}
      variant="ghost"
      size="sm"
      leadingIcon={icons.language}
      aria-label={t("language.current").replace("{name}", name)}
      aria-haspopup="menu"
      {...props}
    >
      <span lang={locale}>{name}</span>
    </Button>
  );
});
