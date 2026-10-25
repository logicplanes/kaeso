import { AgentAvatar, Text, useHarnessName } from "@logicplanes/kaeso-ui";
import { StateCell, StateMatrix, StateRow } from "~/lib/states";

export const meta = {
  title: "AgentAvatar",
  description:
    "An agent acting for a person, as a rounded square with a monochrome glyph for its harness: Claude Code, Codex, Cursor, Gemini CLI, OpenCode, Pi or another.",
  group: "Components",
  entry: "agent-avatar",
  import: "AgentAvatar, useHarnessName",
  a11y: [
    "role=img named after the agent; pass decorative when the name is written next to it.",
    "The glyph only tells harnesses apart at a glance; write the harness's name with useHarnessName where it matters.",
  ],
};
export const examples = [
  { id: "harnesses", title: "Harnesses", fn: "Harnesses" },
  { id: "sizes", title: "Sizes", fn: "Sizes" },
];

const agents = [
  { name: "Release helper", harness: "claude_code" },
  { name: "Migration reviewer", harness: "codex" },
  { name: "Console refactor", harness: "cursor" },
  { name: "Docs translator", harness: "gemini_cli" },
  { name: "Key rotation", harness: "opencode" },
  { name: "Usage digest", harness: "pi" },
  { name: "Nightly triage", harness: "other" },
];

export function Harnesses() {
  const harnessName = useHarnessName();
  return (
    <div className="flex flex-col gap-12">
      {agents.map((a) => (
        <div key={a.harness} className="flex items-center gap-8">
          <AgentAvatar harness={a.harness} name={a.name} decorative />
          <Text>{a.name}</Text>
          <Text variant="small" tone="secondary">
            {harnessName(a.harness)}
          </Text>
        </div>
      ))}
    </div>
  );
}
export function Sizes() {
  return (
    <div className="flex flex-wrap items-end gap-12">
      {([20, 24, 28, 32, 40] as const).map((s) => (
        <AgentAvatar key={s} harness="claude_code" name="Release helper" size={s} />
      ))}
    </div>
  );
}

const SIZES = [20, 24, 28, 32, 40] as const;
export function States() {
  return (
    <StateMatrix title="Every harness at every size" columns={SIZES.map((s) => `${s}px`)}>
      {[...agents, { name: "Unknown harness", harness: "aider" }].map((a) => (
        <StateRow key={a.harness} label={a.harness}>
          {SIZES.map((s) => (
            <StateCell key={s} id={`${a.harness}-${s}`}>
              <AgentAvatar harness={a.harness} name={a.name} size={s} />
            </StateCell>
          ))}
        </StateRow>
      ))}
    </StateMatrix>
  );
}
