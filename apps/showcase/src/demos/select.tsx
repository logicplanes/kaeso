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
const longList: SelectOption[] = [
  { value: "gpt-5", label: "GPT-5", group: "OpenAI" },
  { value: "gpt-5-mini", label: "GPT-5 mini", group: "OpenAI" },
  { value: "gpt-5-nano", label: "GPT-5 nano", group: "OpenAI" },
  { value: "gpt-4.1", label: "GPT-4.1", group: "OpenAI" },
  { value: "o3", label: "o3", group: "OpenAI" },
  { value: "o4-mini", label: "o4-mini", group: "OpenAI" },
  { value: "claude-opus-4-1", label: "Claude Opus 4.1", group: "Anthropic" },
  { value: "claude-sonnet-4-5", label: "Claude Sonnet 4.5", group: "Anthropic" },
  { value: "claude-haiku-4-5", label: "Claude Haiku 4.5", group: "Anthropic" },
  { value: "claude-3-5-haiku", label: "Claude Haiku 3.5 (retired on 19 February 2026)", group: "Anthropic", disabled: true },
  { value: "deepseek-chat", label: "DeepSeek V3.2", group: "DeepSeek" },
  { value: "deepseek-reasoner", label: "DeepSeek V3.2 Reasoner, with thinking turned on for every request", group: "DeepSeek" },
];

export function States() {
  return (
    <>
      <StateSection title="Open list">
        <div className="h-560">
          <Select aria-label="Model" options={longList} defaultValue="deepseek-reasoner" open />
        </div>
      </StateSection>
      <Matrix
        title="Trigger"
        rows={[{ label: "default" }, { label: "ghost" }, { label: "header" }]}
        columns={[
          { label: "Rest", props: {} },
          { label: "Hover", props: { "data-force": "hover" } },
          { label: "Focus", props: { "data-force": "focus" } },
          { label: "Pressed", props: { "data-force": "pressed" } },
        ]}
        render={(r, p) => <Select aria-label="Retention" appearance={r.label as "default"} size="sm" options={retention} defaultValue="7" {...p} />}
      />
      <Matrix
        title="Value"
        rows={[{ label: "default" }, { label: "ghost" }, { label: "header" }]}
        columns={[
          { label: "Open", props: { "data-force": "open" } },
          { label: "Placeholder", props: { defaultValue: null } },
          { label: "Invalid", props: { invalid: true } },
          { label: "Disabled", props: { disabled: true } },
        ]}
        render={(r, p) => <Select aria-label="Retention" appearance={r.label as "default"} size="sm" options={retention} defaultValue="7" {...p} />}
      />
      <Matrix
        title="Sizes"
        rows={(["sm", "md", "lg", "xl"] as const).map((s) => ({ label: s }))}
        columns={[{ label: "Value", props: {} }]}
        render={(r) => <Select aria-label="Retention" size={r.label as "sm"} options={retention} defaultValue="90" />}
      />
      <StateSection title="Errors, placeholders and long values">
        <div className="flex w-280 flex-col gap-16">
          <Select label="Log retention" options={retention} error="Choose how long request logs are kept." fullWidth />
          <Select label="Log retention" options={retention} disabled fullWidth />
          <div className="w-120">
            <Select
              multiple
              aria-label="Providers"
              placeholder="All providers"
              options={[
                { value: "openai", label: "OpenAI" },
                { value: "anthropic", label: "Anthropic" },
                { value: "vertex", label: "Google Vertex AI" },
              ]}
              defaultValue={["vertex", "openai", "anthropic"]}
              fullWidth
            />
          </div>
          <Select aria-label="Model" options={longList} defaultValue="claude-sonnet-4-5" fullWidth />
        </div>
      </StateSection>
    </>
  );
}
