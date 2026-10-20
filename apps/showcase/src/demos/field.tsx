import { Field, Fieldset, Input, Select, Switch } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Field",
  description: "A label, a control and its help, wired together; an error shows under the control.",
  group: "Components",
  entry: "field",
  import: "Field, Fieldset",
  a11y: [
    "The label points at the control; help is in aria-describedby.",
    "invalid sets aria-invalid and the danger ring; error shows what is wrong under the control and is its description for a screen reader.",
    "Optional fields are marked, not required ones.",
  ],
};
export const examples = [
  { id: "vertical", title: "Vertical", fn: "Vertical" },
  { id: "horizontal", title: "Horizontal", fn: "Horizontal" },
];
export function Vertical() {
  return (
    <Fieldset legend="New key" description="Applications use this key to call the gateway." className="w-400">
      <Field label="Name" description="Shown in logs and the audit log.">
        <Input placeholder="checkout-web" />
      </Field>
      <Field label="Expires" optional>
        <Select
          options={[
            { value: "never", label: "Never" },
            { value: "30", label: "In 30 days" },
            { value: "90", label: "In 90 days" },
          ]}
          defaultValue="90"
        />
      </Field>
    </Fieldset>
  );
}
export function Horizontal() {
  return (
    <div className="flex w-640 flex-col gap-24">
      <Field label="Name" layout="horizontal" description="Shown in logs.">
        <Input defaultValue="checkout-web" />
      </Field>
      <Field label="Monthly budget" layout="horizontal" invalid>
        <Input defaultValue="-20" suffix="USD" />
      </Field>
    </div>
  );
}
export function States() {
  return (
    <StateSection title="Layouts">
      <div className="flex flex-col gap-40">
        <Vertical />
        <Horizontal />
        <Switch
          label="Require two-step verification"
          description="Members without it are asked to set it up at their next sign-in."
          defaultChecked
          className="w-400"
        />
      </div>
    </StateSection>
  );
}
