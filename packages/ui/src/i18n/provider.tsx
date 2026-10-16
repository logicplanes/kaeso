import { useMemo, type ReactNode } from "react";
import { setFormatLocale } from "../format/index.js";
import { LocaleContext, type UiMessages } from "./context.js";
import { matchUiLocale, type UiLocale } from "./locale-tags.js";

const NO_MESSAGES: UiMessages = {};

export function LocaleProvider({ locale, messages = NO_MESSAGES, children }: { locale: string; messages?: UiMessages | undefined; children: ReactNode }) {
  setFormatLocale(locale, messages);
  const value = useMemo(() => ({ locale, messages }), [locale, messages]);
  return <LocaleContext.Provider value={value}>{children}</LocaleContext.Provider>;
}

export function resolveUiLocale(locale: string): UiLocale {
  return matchUiLocale(locale) ?? "en";
}

export async function loadUiMessages(locale: string): Promise<UiMessages> {
  const tag = resolveUiLocale(locale);
  if (tag === "de") return (await import("./locales/de.js")).default;
  if (tag === "zh-CN") return (await import("./locales/zh-CN.js")).default;
  return {};
}
