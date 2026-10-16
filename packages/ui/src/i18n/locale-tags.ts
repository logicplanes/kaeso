export const UI_LOCALES = ["en", "de", "zh-CN"] as const;
export type UiLocale = (typeof UI_LOCALES)[number];

export function matchUiLocale(tag: string): UiLocale | undefined {
  const lower = tag.trim().toLowerCase();
  if (lower === "en" || lower.startsWith("en-")) return "en";
  if (lower === "de" || lower.startsWith("de-")) return "de";
  if (lower === "zh" || lower.startsWith("zh-")) return "zh-CN";
  return undefined;
}

export function preferredUiLocale(tags: readonly string[]): UiLocale {
  for (const tag of tags) {
    const locale = matchUiLocale(tag);
    if (locale) return locale;
  }
  return "en";
}
