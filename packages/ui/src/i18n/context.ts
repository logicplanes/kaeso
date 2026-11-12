import { createContext, useContext, useMemo } from "react";
import { formatMessage, type MessageValues } from "./message.js";
import { createFormatters, plainFormatters, type Formatters } from "../format/index.js";

export type UiMessages = Readonly<Record<string, string>>;

export interface LocaleValue {
  locale: string;
  messages: UiMessages;
}

const NO_PROVIDER: LocaleValue = { locale: "en", messages: {} };

export const LocaleContext = createContext<LocaleValue>(NO_PROVIDER);

export function useLocale(): string {
  return useContext(LocaleContext).locale;
}

export type UiText = (key: string, english: string, values?: MessageValues) => string;

export function useUiText(): UiText {
  const { locale, messages } = useContext(LocaleContext);
  return useMemo(() => (key: string, english: string, values?: MessageValues) => formatMessage(messages[key] ?? english, values, locale), [locale, messages]);
}

/** The formatters in the nearest `LocaleProvider`'s language and catalog. Use them in components instead of the plain functions, so a server rendering several requests at once formats each in its own language. Without a provider they are the plain formatters. */
export function useFormatters(): Formatters {
  const value = useContext(LocaleContext);
  return useMemo(() => (value === NO_PROVIDER ? plainFormatters : createFormatters(value)), [value]);
}

export const englishText: UiText = (_key, english, values) => formatMessage(english, values, "en");
