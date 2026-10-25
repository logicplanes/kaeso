export const AGENT_HARNESSES = ["claude_code", "codex", "cursor", "gemini_cli", "opencode", "pi", "other"] as const;

export type AgentHarness = (typeof AGENT_HARNESSES)[number];

/** The harness an API string names, or `other` for anything the library doesn't know. */
export function agentHarness(harness: string): AgentHarness {
  return (AGENT_HARNESSES as readonly string[]).includes(harness) ? (harness as AgentHarness) : "other";
}

// One system for every glyph: a 24 grid, a 16-unit live area (4–20), round caps and joins, stroked in currentColor.
export const HARNESS_GLYPHS: Readonly<Record<AgentHarness, readonly string[]>> = {
  claude_code: ["M4 11 8 15 4 19", "M11 19h4.5", "M17 4.5v2", "M17 9.5v2", "M13.5 8h2", "M18.5 8h2"],
  codex: ["M8 7.5 3.5 12 8 16.5", "M16 7.5l4.5 4.5-4.5 4.5", "M13.5 5.5l-3 13"],
  cursor: ["M6.5 4.5v14l3.6-3.3 2.5 4.8 2.5-1.2-2.4-4.7 4.8-.5z"],
  gemini_cli: ["M10.5 16.5a3 3 0 1 1-6 0a3 3 0 1 1 6 0", "M19.5 7.5a3 3 0 1 1-6 0a3 3 0 1 1 6 0", "M9.6 14.4l4.8-4.8"],
  opencode: ["M9.5 5H6v14h3.5", "M14.5 5H18v14h-3.5"],
  pi: ["M5 7h14", "M9.5 7c0 4-.8 7.4-2.8 10", "M14.5 7v7.8a2.2 2.2 0 0 0 2.2 2.2H18"],
  other: ["M8 8h8a3 3 0 0 1 3 3v5a3 3 0 0 1-3 3H8a3 3 0 0 1-3-3v-5a3 3 0 0 1 3-3z", "M12 5v3", "M9.5 13v1.5", "M14.5 13v1.5"],
};
