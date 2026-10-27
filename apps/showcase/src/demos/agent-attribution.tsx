import { AgentAttribution, Text } from "@logicplanes/kaeso-ui";
import { StateCell, StateMatrix, StateRow, StateSection } from "~/lib/states";

export const meta = {
  title: "AgentAttribution",
  description: "A person's avatar with the agent that acted for them as a small round badge on its corner, for audit rows and activity.",
  group: "Components",
  entry: "agent-avatar",
  import: "AgentAttribution",
  a11y: ["One role=img named after both, such as “Ada Okafor via Release helper (Claude)”, in the page's language."],
};
export const examples = [
  { id: "activity", title: "Activity", fn: "Activity" },
  { id: "sizes", title: "Sizes", fn: "Sizes" },
];

const activity = [
  { person: "Ada Okafor", agent: { name: "Release helper", model: "claude" }, action: "Created key ci-deploy" },
  { person: "Lin Park", agent: { name: "Migration reviewer", model: "gpt" }, action: "Changed the gpt-4.1 route" },
  { person: "Tomás Ruiz", agent: { name: "Nightly triage", model: "deepseek" }, action: "Revoked key staging-old" },
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
        <AgentAttribution key={s} name="Ada Okafor" agent={{ name: "Release helper", model: "claude" }} size={s} />
      ))}
    </div>
  );
}

const SIZES = [20, 24, 28, 32, 40] as const;
const SURFACES = [
  { label: "Panel", className: "" },
  { label: "Row hover", className: "bg-fill-1 controls-on-fill" },
  { label: "Selected row", className: "bg-fill-2 controls-on-fill" },
  { label: "Card", className: "bg-raised shadow-xs" },
];
export function States() {
  return (
    <>
      <StateMatrix title="Every size, with initials and with a picture" columns={SIZES.map((s) => `${s}px`)}>
        <StateRow label="Initials">
          {SIZES.map((s) => (
            <StateCell key={s} id={`agent-${s}`}>
              <AgentAttribution name="Ada Okafor" agent={{ name: "Release helper", model: "claude" }} size={s} />
            </StateCell>
          ))}
        </StateRow>
        <StateRow label="Picture">
          {SIZES.map((s) => (
            <StateCell key={s} id={`picture-${s}`}>
              <AgentAttribution name="Ada Okafor" src="/demos/receipt.svg" agent={{ name: "Release helper", model: "claude" }} size={s} />
            </StateCell>
          ))}
        </StateRow>
      </StateMatrix>
      <StateSection title="On every surface it sits on">
        <div className="flex flex-col gap-8">
          {SURFACES.map((surface) => (
            <div key={surface.label} className={`flex h-44 w-400 items-center justify-between rounded-item px-12 ${surface.className}`}>
              <span className="text-small text-fg-secondary">{surface.label}</span>
              <AgentAttribution name="Lin Park" agent={{ name: "Nightly fixer", model: "gpt" }} size={24} />
            </div>
          ))}
        </div>
      </StateSection>
    </>
  );
}
