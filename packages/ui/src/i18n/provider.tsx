import { useMemo, type ReactNode } from "react";
import { setFormatLocale } from "../format";
import { LocaleContext, type UiMessages } from "./context";

export const UI_LOCALES = ["en", "de", "zh-CN"] as const;
export type UiLocale = (typeof UI_LOCALES)[number];

export function LocaleProvider({ locale, messages = {}, children }: { locale: string; messages?: UiMessages | undefined; children: ReactNode }) {
  setFormatLocale(locale, messages);
  const value = useMemo(() => ({ locale, messages }), [locale, messages]);
  return <LocaleContext.Provider value={value}>{children}</LocaleContext.Provider>;
}

export async function loadUiMessages(locale: string): Promise<UiMessages> {
  const tag = resolveUiLocale(locale);
  if (tag === "de") return (await import("./locales/de")).default;
  if (tag === "zh-CN") return (await import("./locales/zh-CN")).default;
  return {};
}

export function resolveUiLocale(locale: string): UiLocale {
  const lower = locale.toLowerCase();
  if (lower === "de" || lower.startsWith("de-")) return "de";
  if (lower === "zh" || lower.startsWith("zh-")) return "zh-CN";
  return "en";
}
