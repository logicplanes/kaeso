import { formatMessage, preferredUiLocale, UI_LOCALES, type MessageValues, type UiLocale, type UiMessages } from "@logicplanes/kaeso-ui/i18n";
import type { LocaleProvider } from "@logicplanes/kaeso-ui/i18n";
import { readStored, writeStored } from "~/storage";

export type Locale = UiLocale;
export type LocaleChoice = "auto" | Locale;

export type { MessageValues };

let current: Locale = "en";
let messages: UiMessages = {};

export function t(key: string, english: string, values?: MessageValues): string {
  return formatMessage(messages[key] ?? english, values, current);
}

export function locale(): Locale {
  return current;
}

export function resolve(choice: LocaleChoice, preferred: readonly string[] = navigator.languages): Locale {
  return choice === "auto" ? preferredUiLocale(preferred) : choice;
}

const STORAGE_KEY = "kaeso:locale";

export function storedChoice(): LocaleChoice {
  const v = readStored(STORAGE_KEY);
  return UI_LOCALES.find((l) => l === v) ?? "auto";
}

export function storeChoice(choice: LocaleChoice): void {
  writeStored(STORAGE_KEY, choice === "auto" ? null : choice);
}

const catalogs: Record<Exclude<Locale, "en">, () => Promise<{ default: UiMessages }>> = {
  de: () => import("./locales/de"),
  "zh-CN": () => import("./locales/zh-CN"),
};

export interface Loaded {
  locale: Locale;
  messages: UiMessages;
  ui: UiMessages;
  provider: typeof LocaleProvider | undefined;
}

let provider: typeof LocaleProvider | undefined;

export async function load(next: Locale): Promise<Loaded> {
  const english: Loaded = { locale: "en", messages: {}, ui: {}, provider };
  if (next === "en") return english;
  try {
    const [own, { LocaleProvider: libraryProvider, loadUiMessages }] = await Promise.all([catalogs[next](), import("./library")]);
    provider = libraryProvider;
    return { locale: next, messages: own.default, ui: await loadUiMessages(next), provider };
  } catch {
    return english;
  }
}

export function apply(loaded: Loaded): void {
  current = loaded.locale;
  messages = loaded.messages;
  document.documentElement.lang = loaded.locale;
}
