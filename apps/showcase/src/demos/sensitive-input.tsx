import { SensitiveInput } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";
import { FIELD_STATES, StateGrid } from "~/lib/state-grid";

export const meta = {
  title: "SensitiveInput",
  description: "Secrets masked by default, an eye to reveal, mono when revealed, never autocompleted.",
  group: "Components",
  entry: "input",
  import: "SensitiveInput",
  a11y: [
    "The reveal circle is named “Show key” or “Hide key” (“Show password” for passwords).",
    "With Caps Lock on, a password field shows “Caps Lock” beside the reveal circle and announces it once; the field's own description stays.",
    "A revealed key is set in mono at the same size as every other id.",
  ],
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
    <>
      <StateGrid
        title="States"
        kinds={[
          { label: "Masked", id: "masked" },
          { label: "Revealed", id: "revealed" },
        ]}
        states={FIELD_STATES}
        render={(k, p) => (
          <div className="w-240">
            <SensitiveInput aria-label="Provider key" defaultValue="sk-ant-api03-Xf7Lq2" defaultRevealed={k.id === "revealed"} {...p} />
          </div>
        )}
      />
      <StateSection title="Password, with a description">
        <div className="flex w-320 flex-col gap-16">
          <SensitiveInput kind="password" label="Password" description="At least 12 characters." defaultValue="correct horse" />
          <SensitiveInput label="Provider key" description="Only the last four characters stay visible after saving." placeholder="sk-ant-…" />
        </div>
      </StateSection>
    </>
  );
}
