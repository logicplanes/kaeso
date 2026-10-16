import { Select, icons, type SelectOption } from "@logicplanes/kaeso-ui";
import { Matrix, StateSection } from "~/lib/states";

const retention: SelectOption[] = [
  { value: "7", label: "7 days" },
  { value: "30", label: "30 days" },
  { value: "90", label: "90 days" },
  { value: "365", label: "1 year" },
];
const presets: SelectOption[] = [
  { value: "owner", label: "Owner", description: "Everything, including billing and deleting the organization.", group: "Organization" },
  { value: "admin", label: "Admin", description: "Members, projects, provider keys and security.", group: "Organization" },
  { value: "developer", label: "Developer", description: "Keys, models and logs in their projects.", group: "Project" },
  { value: "viewer", label: "Viewer", description: "Read usage and logs without content.", group: "Project" },
];

export const meta = {
  title: "Select",
  description: "A pill that opens a list on the menu surface: a check on the selected item, groups, typeahead.",
  group: "Components",
  entry: "select",
  import: "Select",
  keyboard: [
    ["Enter Space", "Opens"],
    ["Up Down", "Moves"],
    ["Type", "Jumps to an option"],
    ["Escape", "Closes and returns focus"],
  ],
  a11y: [
    "Takes label, description and error like Input.",
    'appearance="ghost" is for inline use in settings rows, where the row title is the label: pass aria-label.',
  ],
};
export const examples = [
  { id: "basic", title: "Basic", fn: "Basic" },
  { id: "ghost", title: "Ghost, in a settings row", fn: "Ghost" },
  { id: "rich", title: "Descriptions and groups", fn: "Rich" },
  { id: "multiple", title: "Multiple", fn: "Multiple" },
];
export function Basic() {
  return (
    <div className="w-280">
      <Select label="Log retention" options={retention} defaultValue="30" fullWidth />
    </div>
  );
}
export function Ghost() {
  return (
    <Select
      appearance="ghost"
      size="sm"
      aria-label="First day of the week"
      options={[
        { value: "mon", label: "Monday" },
        { value: "sun", label: "Sunday" },
      ]}
      defaultValue="mon"
    />
  );
}
export function Rich() {
  return <Select aria-label="Preset" options={presets} defaultValue="developer" icon={icons.members} />;
}
export function Multiple() {
  return (
    <Select
      multiple
      aria-label="Providers"
      placeholder="All providers"
      options={[
        { value: "openai", label: "OpenAI" },
        { value: "anthropic", label: "Anthropic" },
      ]}
      defaultValue={["openai"]}
    />
  );
}
export function States() {
  return (
    <>
      <Matrix
        title="Trigger"
        rows={[{ label: "default" }, { label: "ghost" }]}
        columns={[
          { label: "Rest", props: {} },
          { label: "Hover", props: { "data-force": "hover" } },
          { label: "Focus", props: { "data-force": "focus" } },
          { label: "Open", props: { "data-force": "open" } },
          { label: "Placeholder", props: { defaultValue: null } },
          { label: "Invalid", props: { invalid: true } },
          { label: "Disabled", props: { disabled: true } },
        ]}
        render={(r, p) => (
          <Select aria-label="Retention" appearance={r.label as "default"} size="sm" options={retention} defaultValue="30" placeholder="Choose" {...p} />
        )}
      />
      <Matrix
        title="Sizes"
        rows={(["sm", "md", "lg", "xl"] as const).map((s) => ({ label: s }))}
        columns={[{ label: "Value", props: {} }]}
        render={(r) => <Select aria-label="Retention" size={r.label as "sm"} options={retention} defaultValue="90" />}
      />
      <StateSection title="Open list">
        <div className="h-320">
          <Select aria-label="Preset" options={presets} defaultValue="developer" open />
        </div>
      </StateSection>
    </>
  );
}
