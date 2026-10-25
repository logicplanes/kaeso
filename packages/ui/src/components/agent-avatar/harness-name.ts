import { useMemo } from "react";
import { englishText, useUiText, type UiText } from "../../i18n/context.js";
import { agentHarness, type AgentHarness } from "./harness-glyphs.js";

const PRODUCT_NAMES: Readonly<Record<Exclude<AgentHarness, "other">, string>> = {
  claude_code: "Claude Code",
  codex: "Codex",
  cursor: "Cursor",
  gemini_cli: "Gemini CLI",
  opencode: "OpenCode",
  pi: "Pi",
};

/**
 * The display name of an agent harness from the API, such as `claude_code` → "Claude Code". Product names are never
 * translated; `other` and unknown harnesses are "Other", translated through `t` (English without it). In a component,
 * prefer `useHarnessName()`, which translates into the page's language.
 */
export function harnessName(harness: string, t: UiText = englishText): string {
  const known = agentHarness(harness);
  return known === "other" ? t("agentAvatar.otherHarness", "Other") : PRODUCT_NAMES[known];
}

/** `harnessName` in the language of the nearest `LocaleProvider`. */
export function useHarnessName(): (harness: string) => string {
  const t = useUiText();
  return useMemo(() => (harness: string) => harnessName(harness, t), [t]);
}
