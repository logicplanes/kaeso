import { Checkbox, CheckboxGroup } from "@logicplanes/kaeso-ui";
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
        <div className="flex w-400 flex-col">
          <Checkbox label="Log request content" defaultChecked />
          <Checkbox label="Disabled" disabled />
          <Checkbox
            label="A long label that wraps onto a second line keeps its box aligned with the first line of text"
            description="And a description under it."
          />
        </div>
      </StateSection>
    </>
  );
}
