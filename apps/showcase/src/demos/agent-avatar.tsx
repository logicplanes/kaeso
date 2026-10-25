import { AGENT_MODELS, AgentAvatar, Text, useModelName } from "@logicplanes/kaeso-ui";
import { StateCell, StateMatrix, StateRow } from "~/lib/states";

export const meta = {
  title: "AgentAvatar",
  description:
    "An agent acting for a person, as a rounded square with a monochrome glyph for the model family it reports, from Claude and GPT to DeepSeek and Qwen.",
  group: "Components",
  entry: "agent-avatar",
  import: "AgentAvatar, useModelName",
  a11y: [
    "role=img named after the agent; pass decorative when the name is written next to it.",
    "The glyph only tells model families apart at a glance; write the family's name with useModelName where it matters.",
  ],
};
export const examples = [
  { id: "agents", title: "Agents", fn: "Agents" },
  { id: "model-families", title: "Model families", fn: "ModelFamilies" },
  { id: "sizes", title: "Sizes", fn: "Sizes" },
];

const agents = [
  { name: "Release helper", model: "claude" },
  { name: "Migration reviewer", model: "gpt" },
  { name: "Docs translator", model: "gemini" },
  { name: "Key rotation", model: "deepseek" },
  { name: "Usage digest", model: "qwen" },
  { name: "Nightly triage", model: "unknown" },
];

export function Agents() {
  const modelName = useModelName();
  return (
    <div className="flex flex-col gap-12">
      {agents.map((a) => (
        <div key={a.model} className="flex items-center gap-8">
          <AgentAvatar model={a.model} name={a.name} decorative />
          <Text>{a.name}</Text>
          <Text variant="small" tone="secondary">
            {modelName(a.model)}
          </Text>
        </div>
      ))}
    </div>
  );
}
export function ModelFamilies() {
  const modelName = useModelName();
  return (
    <div className="grid grid-cols-2 gap-x-24 gap-y-12 sm:grid-cols-3">
      {AGENT_MODELS.map((m) => (
        <div key={m} className="flex items-center gap-8">
          <AgentAvatar model={m} name={modelName(m)} decorative />
          <Text variant="small">{modelName(m)}</Text>
        </div>
      ))}
    </div>
  );
}
export function Sizes() {
  return (
    <div className="flex flex-wrap items-end gap-12">
      {([20, 24, 28, 32, 40] as const).map((s) => (
        <AgentAvatar key={s} model="claude" name="Release helper" size={s} />
      ))}
    </div>
  );
}

const SIZES = [20, 24, 28, 32, 40] as const;
export function States() {
  return (
    <StateMatrix title="Every model family at every size" columns={SIZES.map((s) => `${s}px`)}>
      {[...AGENT_MODELS, "falcon"].map((m) => (
        <StateRow key={m} label={m}>
          {SIZES.map((s) => (
            <StateCell key={s} id={`${m}-${s}`}>
              <AgentAvatar model={m} name={`Agent on ${m}`} size={s} />
            </StateCell>
          ))}
        </StateRow>
      ))}
    </StateMatrix>
  );
}
