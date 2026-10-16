import { createContext, useContext, useMemo } from "react";
import { formatMessage, type MessageValues } from "./message.js";

export type UiMessages = Readonly<Record<string, string>>;

export interface LocaleValue {
  locale: string;
  messages: UiMessages;
}

export const LocaleContext = createContext<LocaleValue>({ locale: "en", messages: {} });

export function useLocale(): string {
  return useContext(LocaleContext).locale;
}

export type UiText = (key: string, english: string, values?: MessageValues) => string;

export function useUiText(): UiText {
  const { locale, messages } = useContext(LocaleContext);
  return useMemo(() => (key: string, english: string, values?: MessageValues) => formatMessage(messages[key] ?? english, values, locale), [locale, messages]);
}

export const englishText: UiText = (_key, english, values) => formatMessage(english, values, "en");
