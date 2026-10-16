import { OTPInput, Field } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "OTPInput",
  description: "Six cells for a TOTP code over one accessible input: paste fills, Backspace moves back.",
  group: "Components",
  entry: "input",
  import: "OTPInput",
  a11y: ['One real input with autocomplete="one-time-code", so the browser can offer the code.', "Cells are decorative; the input carries the label."],
};
export const examples = [{ id: "basic", title: "Basic", fn: "Basic" }];
export function Basic() {
  return (
    <Field label="Code from your authenticator app">
      <OTPInput />
    </Field>
  );
}
export function States() {
  return (
    <StateSection title="States">
      <div className="flex flex-col items-start gap-16">
        <OTPInput aria-label="Empty" />
        <OTPInput aria-label="Focused" data-force="focus" />
        <OTPInput aria-label="Partial" defaultValue="418" data-force="focus" />
        <OTPInput aria-label="Complete" defaultValue="418207" />
        <OTPInput aria-label="Invalid" defaultValue="418207" invalid />
        <OTPInput aria-label="Disabled" disabled />
      </div>
    </StateSection>
  );
}
