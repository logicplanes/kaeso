import { AgentAttribution, Text } from "@logicplanes/kaeso-ui";
import { StateCell, StateMatrix, StateRow } from "~/lib/states";

export const meta = {
  title: "AgentAttribution",
  description: "A person's avatar with the agent that acted for them as a badge on its corner, for audit rows and activity.",
  group: "Components",
  entry: "agent-avatar",
  import: "AgentAttribution",
  a11y: ["One role=img named after both, such as “Ada Okafor via Release helper (Claude Code)”, in the page's language."],
};
export const examples = [
  { id: "activity", title: "Activity", fn: "Activity" },
  { id: "sizes", title: "Sizes", fn: "Sizes" },
];

const activity = [
  { person: "Ada Okafor", agent: { name: "Release helper", harness: "claude_code" }, action: "Created key ci-deploy" },
  { person: "Lin Park", agent: { name: "Migration reviewer", harness: "codex" }, action: "Changed the gpt-4.1 route" },
  { person: "Tomás Ruiz", agent: { name: "Nightly triage", harness: "other" }, action: "Revoked key staging-old" },
];

export function Activity() {
  return (
    <div className="flex flex-col gap-16">
      {activity.map((a) => (
        <div key={a.person} className="flex items-center gap-12">
          <AgentAttribution name={a.person} agent={a.agent} />
          <Text>{a.action}</Text>
        </div>
      ))}
    </div>
  );
}
export function Sizes() {
  return (
    <div className="flex flex-wrap items-end gap-16">
      {([20, 24, 28, 32, 40] as const).map((s) => (
        <AgentAttribution key={s} name="Ada Okafor" agent={{ name: "Release helper", harness: "claude_code" }} size={s} />
      ))}
    </div>
  );
}

const SIZES = [20, 24, 28, 32, 40] as const;
const HARNESSES = ["claude_code", "codex", "cursor", "gemini_cli", "opencode", "pi", "other"];
export function States() {
  return (
    <StateMatrix title="Every harness at every size" columns={SIZES.map((s) => `${s}px`)}>
      {HARNESSES.map((h) => (
        <StateRow key={h} label={h}>
          {SIZES.map((s) => (
            <StateCell key={s} id={`${h}-${s}`}>
              <AgentAttribution name="Ada Okafor" agent={{ name: "Release helper", harness: h }} size={s} />
            </StateCell>
          ))}
        </StateRow>
      ))}
    </StateMatrix>
  );
}
