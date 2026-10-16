import { Switch } from "@logicplanes/kaeso-ui";
import { Matrix, StateSection } from "~/lib/states";

export const meta = {
  title: "Switch",
  description: "A pill track and a round thumb that travels in 180ms.",
  group: "Components",
  entry: "switch",
  import: "Switch",
  keyboard: [["Space", "Toggles"]],
  a11y: ["Use a switch for settings that apply at once; use a checkbox inside forms that are submitted."],
};
export const examples = [
  { id: "basic", title: "Basic", fn: "Basic" },
  { id: "sizes", title: "Sizes", fn: "Sizes" },
  { id: "reason", title: "Disabled, with the reason", fn: "Reason" },
];
export function Basic() {
  return <Switch label="Send budget alerts" description="Email owners at 80% and 100% of the monthly budget." defaultChecked className="w-400" />;
}
export function Sizes() {
  return (
    <div className="flex items-center gap-16">
      <Switch size="sm" aria-label="Small" defaultChecked />
      <Switch size="md" aria-label="Medium" defaultChecked />
      <Switch size="lg" aria-label="Large" defaultChecked />
    </div>
  );
}
export function Reason() {
  return (
    <Switch
      label="Require single sign-on"
      description="Members reach the organization only through the identity provider."
      disabled
      disabledReason="Connect an identity provider first"
      className="w-400"
    />
  );
}
export function States() {
  return (
    <>
      <Matrix
        title="Off and on"
        rows={[{ label: "off" }, { label: "on" }]}
        columns={[
          { label: "Rest", props: {} },
          { label: "Hover", props: { "data-force": "hover" } },
          { label: "Focus", props: { "data-force": "focus" } },
          { label: "Loading", props: { loading: true } },
          { label: "Disabled", props: { disabled: true } },
          { label: "With reason", props: { disabled: true, disabledReason: "Connect an identity provider first" } },
        ]}
        render={(r, p) => <Switch aria-label={r.label} defaultChecked={r.label === "on"} {...p} />}
      />
      <StateSection title="Sizes">
        <Sizes />
      </StateSection>
    </>
  );
}
