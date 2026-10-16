import { formatMessage, type MessageValues, type UiMessages } from "@logicplanes/kaeso-ui/i18n";
import type { LocaleProvider } from "@logicplanes/kaeso-ui/i18n";
import { readStored, writeStored } from "~/storage";

export type Locale = "en" | "de" | "zh-CN";
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

export function supported(tag: string): Locale | undefined {
  const lower = tag.toLowerCase();
  if (lower === "en" || lower.startsWith("en-")) return "en";
  if (lower === "de" || lower.startsWith("de-")) return "de";
  if (lower === "zh" || lower.startsWith("zh-")) return "zh-CN";
  return undefined;
}

export function resolve(choice: LocaleChoice, preferred: readonly string[] = navigator.languages): Locale {
  if (choice !== "auto") return choice;
  for (const tag of preferred) {
    const found = supported(tag);
    if (found) return found;
  }
  return "en";
}

const STORAGE_KEY = "kaeso:locale";

export function storedChoice(): LocaleChoice {
  const v = readStored(STORAGE_KEY);
  return v === "en" || v === "de" || v === "zh-CN" ? v : "auto";
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
