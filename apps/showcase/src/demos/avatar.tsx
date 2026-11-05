import { Avatar, Text, useModelName } from "@logicplanes/kaeso-ui";
import { members } from "~/fixtures/kaeso";
import { StateCell, StateMatrix, StateRow, StateSection } from "~/lib/states";

export const meta = {
  title: "Avatar",
  description: "People round with initials or a picture; organizations and projects at radius 8; agents as a bot face on a square.",
  group: "Components",
  entry: "avatar",
  import: "Avatar",
  a11y: [
    "role=img named after the person, organization or agent; pass decorative when the name is written next to it.",
    "A person acting through an agent is one role=img named after both, such as “Ada Okafor via Release helper (Claude)”, in the page's language.",
    "Every agent looks the same; write its model family with useModelName where it matters.",
  ],
};
export const examples = [
  { id: "sizes", title: "Sizes", fn: "Sizes" },
  { id: "group", title: "Group", fn: "Group" },
  { id: "agent-agents", title: "Agents", fn: "AgentAgents", description: '`kind="agent"`: one bot face on a square, the same for every agent.' },
  { id: "agent-sizes", title: "Agent sizes", fn: "AgentSizes" },
  {
    id: "attribution-activity",
    title: "Acting through an agent",
    fn: "AttributionActivity",
    description: "`agent`: a person's avatar with the agent that acted for them as a badge on its corner.",
  },
  { id: "attribution-sizes", title: "Acting through an agent, sizes", fn: "AttributionSizes" },
];
export function Sizes() {
  return (
    <div className="flex flex-wrap items-end gap-12">
      {([20, 24, 28, 32, 40] as const).map((s) => (
        <Avatar key={s} name="Ada Okafor" size={s} />
      ))}
      {([20, 24, 32, 40] as const).map((s) => (
        <Avatar key={`o${s}`} name="Northwind Labs" kind="org" size={s} />
      ))}
    </div>
  );
}
export function Group() {
  return <Avatar.Group people={members} max={3} size={28} />;
}
const SURFACES = [
  { label: "Panel", className: "" },
  { label: "Row hover", className: "bg-fill-1 controls-on-fill" },
  { label: "Selected row", className: "bg-fill-2 controls-on-fill" },
  { label: "Card", className: "bg-raised shadow-xs" },
];
export function States() {
  return (
    <>
      <StateSection title="Sizes; a picture, a picture that failed, one-word and Chinese names">
        <div className="flex flex-col gap-16">
          <Sizes />
          <div className="flex items-center gap-12">
            <Avatar name="Ada Okafor" src="/demos/receipt.svg" size={32} />
            <Avatar name="Lin Park" src="/demos/missing-picture.png" size={32} />
            <Avatar name="Northwind" size={32} />
            <Avatar name="李娜" size={32} />
            <Avatar name="Zhang Wei" size={32} />
          </div>
        </div>
      </StateSection>
      <StateSection title="Groups on every surface they sit on">
        <div className="flex flex-col gap-8">
          {SURFACES.map((surface) => (
            <div key={surface.label} className={`flex h-44 w-400 items-center justify-between rounded-item px-12 ${surface.className}`}>
              <span className="text-small text-fg-secondary">{surface.label}</span>
              <Avatar.Group people={members} max={3} size={24} />
            </div>
          ))}
          <div className="flex items-center gap-16">
            {([20, 24, 28, 32, 40] as const).map((s) => (
              <Avatar.Group key={s} people={members} max={2} size={s} />
            ))}
          </div>
        </div>
      </StateSection>
      <AgentStates />
      <AttributionStates />
    </>
  );
}

const Agentagents = [
  { name: "Release helper", model: "claude" },
  { name: "Migration reviewer", model: "gpt" },
  { name: "Nightly triage", model: "unknown" },
];

export function AgentAgents() {
  const modelName = useModelName();
  return (
    <div className="flex flex-col gap-12">
      {Agentagents.map((a) => (
        <div key={a.model} className="flex items-center gap-8">
          <Avatar kind="agent" name={a.name} decorative />
          <Text>{a.name}</Text>
          <Text variant="small" tone="secondary">
            {modelName(a.model)}
          </Text>
        </div>
      ))}
    </div>
  );
}
export function AgentSizes() {
  return (
    <div className="flex flex-wrap items-end gap-12">
      {([20, 24, 28, 32, 40] as const).map((s) => (
        <Avatar kind="agent" key={s} name="Release helper" size={s} />
      ))}
    </div>
  );
}

const AgentSIZES = [20, 24, 28, 32, 40] as const;
function AgentStates() {
  return (
    <>
      <StateMatrix title="Every size" columns={AgentSIZES.map((s) => `${s}px`)}>
        <StateRow label="Agent">
          {AgentSIZES.map((s) => (
            <StateCell key={s} id={`agent-${s}`}>
              <Avatar kind="agent" name="Release helper" size={s} />
            </StateCell>
          ))}
        </StateRow>
      </StateMatrix>
      <StateSection title="Beside people, in a member list">
        <div className="flex items-center gap-8">
          <Avatar name="Ada Okafor" size={24} />
          <Avatar name="Lin Park" size={24} />
          <Avatar kind="agent" name="Release helper" size={24} />
          <Avatar name="Tomás Ruiz" size={24} />
        </div>
      </StateSection>
    </>
  );
}

const Attributionactivity = [
  { person: "Ada Okafor", agent: { name: "Release helper", model: "claude" }, action: "Created key ci-deploy" },
  { person: "Lin Park", agent: { name: "Migration reviewer", model: "gpt" }, action: "Changed the gpt-4.1 route" },
  { person: "Tomás Ruiz", agent: { name: "Nightly triage", model: "deepseek" }, action: "Revoked key staging-old" },
];

export function AttributionActivity() {
  return (
    <div className="flex flex-col gap-16">
      {Attributionactivity.map((a) => (
        <div key={a.person} className="flex items-center gap-12">
          <Avatar name={a.person} agent={a.agent} />
          <Text>{a.action}</Text>
        </div>
      ))}
    </div>
  );
}
export function AttributionSizes() {
  return (
    <div className="flex flex-wrap items-end gap-16">
      {([20, 24, 28, 32, 40] as const).map((s) => (
        <Avatar key={s} name="Ada Okafor" agent={{ name: "Release helper", model: "claude" }} size={s} />
      ))}
    </div>
  );
}

const AttributionSIZES = [20, 24, 28, 32, 40] as const;
const AttributionSURFACES = [
  { label: "Panel", className: "" },
  { label: "Row hover", className: "bg-fill-1 controls-on-fill" },
  { label: "Selected row", className: "bg-fill-2 controls-on-fill" },
  { label: "Card", className: "bg-raised shadow-xs" },
];
function AttributionStates() {
  return (
    <>
      <StateMatrix title="Every size, with initials and with a picture" columns={AttributionSIZES.map((s) => `${s}px`)}>
        <StateRow label="Initials">
          {AttributionSIZES.map((s) => (
            <StateCell key={s} id={`agent-${s}`}>
              <Avatar name="Ada Okafor" agent={{ name: "Release helper", model: "claude" }} size={s} />
            </StateCell>
          ))}
        </StateRow>
        <StateRow label="Picture">
          {AttributionSIZES.map((s) => (
            <StateCell key={s} id={`picture-${s}`}>
              <Avatar name="Ada Okafor" src="/demos/receipt.svg" agent={{ name: "Release helper", model: "claude" }} size={s} />
            </StateCell>
          ))}
        </StateRow>
      </StateMatrix>
      <StateSection title="On every surface it sits on">
        <div className="flex flex-col gap-8">
          {AttributionSURFACES.map((surface) => (
            <div key={surface.label} className={`flex h-44 w-400 items-center justify-between rounded-item px-12 ${surface.className}`}>
              <span className="text-small text-fg-secondary">{surface.label}</span>
              <Avatar name="Lin Park" agent={{ name: "Nightly fixer", model: "gpt" }} size={24} />
            </div>
          ))}
        </div>
      </StateSection>
    </>
  );
}
