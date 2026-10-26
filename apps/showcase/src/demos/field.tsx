import { Field, Fieldset, Input, Select, Switch, Textarea } from "@logicplanes/kaeso-ui";
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
    "Fieldset's legend names the group, so each field inside is heard with it; its description describes the group.",
    "After a failed submit, move focus to the first invalid field: its error is then read with it.",
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
      <Field label="Monthly budget" layout="horizontal" error="Enter an amount of 0 or more.">
        <Input defaultValue="-20" suffix="USD" align="end" />
      </Field>
    </div>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Vertical, with every part">
        <div className="flex w-400 flex-col gap-24">
          <Vertical />
          <Field label="Monthly budget" description="Requests stop once this is spent." error="Enter an amount of 0 or more.">
            <Input defaultValue="-20" suffix="USD" align="end" />
          </Field>
          <Field label="Rate limit" description="Set by your organization." disabled>
            <Input defaultValue="600" suffix="/ min" disabled />
          </Field>
        </div>
      </StateSection>
      <StateSection title="Horizontal: the label sits on the control's text line">
        <div className="flex w-640 flex-col gap-24">
          {(["sm", "md", "lg", "xl"] as const).map((s) => (
            <Field key={s} label={`Name (${s})`} layout="horizontal">
              <Input size={s} defaultValue="checkout-web" />
            </Field>
          ))}
          <Field label="System prompt" layout="horizontal" description="Sent before every message.">
            <Textarea defaultValue="You help customers finish checkout." />
          </Field>
          <Field label="Monthly budget" layout="horizontal" error="Enter an amount of 0 or more.">
            <Input defaultValue="-20" suffix="USD" align="end" />
          </Field>
        </div>
      </StateSection>
      <StateSection title="Long labels in German">
        <div className="flex w-640 flex-col gap-24">
          <Field label="Höchstbetrag pro Monat für alle Schlüssel dieses Projekts" layout="horizontal" optional description="Anfragen werden danach abgelehnt.">
            <Input defaultValue="1.250,00" suffix="EUR" align="end" />
          </Field>
          <div className="w-240">
            <Field label="Weiterleitungsadresse für Benachrichtigungen" optional>
              <Input placeholder="ops@northwind.example" />
            </Field>
          </div>
        </div>
      </StateSection>
      <StateSection title="Beside a switch">
        <Switch
          label="Require two-step verification"
          description="Members without it are asked to set it up at their next sign-in."
          defaultChecked
          className="w-400"
        />
      </StateSection>
    </>
  );
}
