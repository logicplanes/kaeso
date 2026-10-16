import { SensitiveInput } from "@logicplanes/kaeso-ui";
import { Matrix } from "~/lib/states";

export const meta = {
  title: "SensitiveInput",
  description: "Secrets masked by default, an eye to reveal, mono when revealed, never autocompleted.",
  group: "Components",
  entry: "input",
  import: "SensitiveInput",
  a11y: ["The reveal circle is named Show or Hide.", "Passwords show “Caps Lock is on.” in the field's description."],
};
export const examples = [
  { id: "secret", title: "Provider key", fn: "Secret" },
  { id: "password", title: "Password", fn: "Password" },
];
export function Secret() {
  return (
    <div className="w-400">
      <SensitiveInput label="Provider key" description="Only the last four characters stay visible after saving." placeholder="sk-ant-…" />
    </div>
  );
}
export function Password() {
  return (
    <div className="w-320">
      <SensitiveInput kind="password" label="Password" />
    </div>
  );
}
export function States() {
  return (
    <Matrix
      title="States"
      rows={[{ label: "masked" }, { label: "revealed" }]}
      columns={[
        { label: "Rest", props: {} },
        { label: "Focus", props: { "data-force": "focus" } },
        { label: "Invalid", props: { invalid: true } },
        { label: "Disabled", props: { disabled: true } },
      ]}
      render={(r, p) => (
        <div className="w-240">
          <SensitiveInput aria-label="Provider key" defaultValue="sk-ant-api03-Xf7Lq2" defaultRevealed={r.label === "revealed"} {...p} />
        </div>
      )}
    />
  );
}
