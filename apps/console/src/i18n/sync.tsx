import { useEffect } from "react";
import { useSession } from "~/auth/session";
import { storedChoice } from "./index";
import { useChangeLocale } from "./provider";

export function LocaleSync() {
  const { locale } = useSession();
  const change = useChangeLocale();
  useEffect(() => {
    if (locale !== storedChoice()) void change(locale);
  }, [locale, change]);
  return null;
}
