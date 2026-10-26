import { AgentAvatar, Avatar, Text, useModelName } from "@logicplanes/kaeso-ui";
import { StateCell, StateMatrix, StateRow, StateSection } from "~/lib/states";

export const meta = {
  title: "AgentAvatar",
  description: "An agent acting for a person: one bot face on a square, the same for every agent.",
  group: "Components",
  entry: "agent-avatar",
  import: "AgentAvatar, useModelName",
  a11y: [
    "role=img named after the agent; pass decorative when the name is written next to it.",
    "Every agent looks the same; write its model family with useModelName where it matters.",
  ],
};
export const examples = [
  { id: "agents", title: "Agents", fn: "Agents" },
  { id: "sizes", title: "Sizes", fn: "Sizes" },
];

const agents = [
  { name: "Release helper", model: "claude" },
  { name: "Migration reviewer", model: "gpt" },
  { name: "Nightly triage", model: "unknown" },
];

export function Agents() {
  const modelName = useModelName();
  return (
    <div className="flex flex-col gap-12">
      {agents.map((a) => (
        <div key={a.model} className="flex items-center gap-8">
          <AgentAvatar name={a.name} decorative />
          <Text>{a.name}</Text>
          <Text variant="small" tone="secondary">
            {modelName(a.model)}
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
        <AgentAvatar key={s} name="Release helper" size={s} />
      ))}
    </div>
  );
}

const SIZES = [20, 24, 28, 32, 40] as const;
export function States() {
  return (
    <>
      <StateMatrix title="Every size" columns={SIZES.map((s) => `${s}px`)}>
        <StateRow label="Agent">
          {SIZES.map((s) => (
            <StateCell key={s} id={`agent-${s}`}>
              <AgentAvatar name="Release helper" size={s} />
            </StateCell>
          ))}
        </StateRow>
      </StateMatrix>
      <StateSection title="Beside people, in a member list">
        <div className="flex items-center gap-8">
          <Avatar name="Ada Okafor" size={24} />
          <Avatar name="Lin Park" size={24} />
          <AgentAvatar name="Release helper" size={24} />
          <Avatar name="Tomás Ruiz" size={24} />
        </div>
      </StateSection>
    </>
  );
}
