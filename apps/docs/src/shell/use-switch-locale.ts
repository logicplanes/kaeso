import { useCallback } from "react";
import { useLocation, useNavigate } from "@tanstack/react-router";
import { switchLocale, type Locale } from "~/i18n/locales";
import { rememberLocale } from "~/i18n/preference";

export function useSwitchLocale(): (locale: Locale) => void {
  const navigate = useNavigate();
  const pathname = useLocation({ select: (l) => l.pathname });
  return useCallback(
    (locale: Locale) => {
      rememberLocale(locale);
      void navigate({ to: switchLocale(pathname, locale) });
    },
    [navigate, pathname],
  );
}
