import { useState } from "react";
import { Checkbox, CheckboxGroup, Field } from "@logicplanes/kaeso-ui";
import { Matrix, StateSection } from "~/lib/states";

export const meta = {
  title: "Checkbox",
  description: "An 18px box: raised and ringed, ink when checked, with a check that draws itself.",
  group: "Components",
  entry: "checkbox",
  import: "Checkbox, CheckboxGroup",
  keyboard: [["Space", "Toggles"]],
  a11y: ["The whole row is the target: clicking the label toggles it.", "Indeterminate is announced as mixed."],
};
export const examples = [
  { id: "basic", title: "Basic", fn: "Basic" },
  { id: "group", title: "Group", fn: "Group" },
];
export function Basic() {
  return <Checkbox label="Log request content" description="Prompts and completions are stored for 30 days." defaultChecked />;
}
export function Group() {
  return (
    <CheckboxGroup defaultValue={["text"]} aria-label="Input types">
      <Checkbox value="text" label="Text" />
      <Checkbox value="image" label="Images" />
      <Checkbox value="pdf" label="PDF" />
    </CheckboxGroup>
  );
}
export function States() {
  return (
    <>
      <Matrix
        title="Box"
        rows={[{ label: "unchecked" }, { label: "checked" }, { label: "indeterminate" }, { label: "invalid" }]}
        render={(r, p) => (
          <Checkbox
            aria-label={r.label}
            {...(r.label === "checked" ? { defaultChecked: true } : {})}
            {...(r.label === "indeterminate" ? { indeterminate: true } : {})}
            {...(r.label === "invalid" ? { invalid: true } : {})}
            {...p}
          />
        )}
      />
      <StateSection title="With labels">
        <div className="flex w-400 flex-col gap-8">
          <Checkbox label="Log request content" description="Prompts and completions are stored for 30 days." defaultChecked />
          <Checkbox label="Hovering the label" description="The row is the target, so the box answers the pointer." data-force="hover" />
          <Checkbox label="Store images" description="Unavailable while request content is off." disabled />
          <Checkbox label="Store tool calls" description="Unavailable on this plan." disabled defaultChecked />
          <Checkbox label="Redact secrets" description="Set by the organization's policy." readOnly defaultChecked />
          <Checkbox
            label="A long label that wraps onto a second line keeps its box aligned with the first line of text"
            description="And a description under it."
          />
          <Field error="Accept the data processing terms to create the organization.">
            <Checkbox label="I accept the data processing terms" />
          </Field>
        </div>
      </StateSection>
      <StateSection title="Group with a parent">
        <GroupWithParent />
      </StateSection>
    </>
  );
}

function GroupWithParent() {
  const all = ["text", "image", "pdf"];
  const [value, setValue] = useState(["text"]);
  return (
    <CheckboxGroup value={value} onValueChange={setValue} allValues={all} aria-label="Input types">
      <label htmlFor="input-types-all" className="inline-flex min-h-32 cursor-pointer items-center gap-10 select-none">
        <Checkbox.Box parent id="input-types-all" aria-labelledby="input-types-all-name" />
        <span id="input-types-all-name" className="text-body text-fg">
          All input types
        </span>
      </label>
      <div className="flex flex-col pl-28">
        <Checkbox value="text" label="Text" />
        <Checkbox value="image" label="Images" />
        <Checkbox value="pdf" label="PDF" />
      </div>
    </CheckboxGroup>
  );
}
