import { createContext, use, useMemo, type ReactNode } from "react";
import { DEFAULT_LOCALE, type Locale } from "./locales";
import { MESSAGES, type MessageKey } from "./messages";

export type Translate = (key: MessageKey) => string;

interface TextValue {
  locale: Locale;
  t: Translate;
}

export function translator(locale: Locale): Translate {
  const messages = MESSAGES[locale];
  return (key) => messages[key];
}

const TextContext = createContext<TextValue>({ locale: DEFAULT_LOCALE, t: translator(DEFAULT_LOCALE) });

export function TextProvider({ locale, children }: { locale: Locale; children: ReactNode }) {
  const value = useMemo(() => ({ locale, t: translator(locale) }), [locale]);
  return <TextContext value={value}>{children}</TextContext>;
}

export function useText(): Translate {
  return use(TextContext).t;
}

export function useSiteLocale(): Locale {
  return use(TextContext).locale;
}
