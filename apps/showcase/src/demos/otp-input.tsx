import { OTPInput, Field } from "@logicplanes/kaeso-ui";
import { StateCell, StateMatrix, StateRow, StateSection } from "~/lib/states";

export const meta = {
  title: "OTPInput",
  description: "Six cells for a TOTP code over one accessible input: paste fills, Backspace moves back.",
  keyboard: [
    ["0–9", "Types a digit into the focused cell and moves to the next"],
    ["Backspace", "Clears the focused digit"],
    ["← →", "Moves between cells"],
    ["Home, End", "Moves to the first cell, or the next empty one"],
  ],
  group: "Components",
  entry: "input",
  import: "OTPInput",
  a11y: [
    'One real input with autocomplete="one-time-code", so the browser can offer the code.',
    "Cells are decorative; the input carries the label.",
    "A pasted code fills every cell even when it is shown with spaces or dashes, such as 418 207.",
    "Clicking a cell puts the caret there, so one wrong digit can be fixed without retyping the code.",
  ],
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
    <>
      <StateMatrix title="States" columns={["Default size"]}>
        <StateRow label="Empty">
          <StateCell id="otp-rest">
            <OTPInput aria-label="Empty" />
          </StateCell>
        </StateRow>
        <StateRow label="Focused">
          <StateCell>
            <OTPInput aria-label="Focused" data-force="focus" />
          </StateCell>
        </StateRow>
        <StateRow label="Partial, focused">
          <StateCell>
            <OTPInput aria-label="Partial" defaultValue="418" data-force="focus" />
          </StateCell>
        </StateRow>
        <StateRow label="Complete">
          <StateCell>
            <OTPInput aria-label="Complete" defaultValue="418207" />
          </StateCell>
        </StateRow>
        <StateRow label="Complete, focused">
          <StateCell>
            <OTPInput aria-label="Complete, focused" defaultValue="418207" data-force="focus" />
          </StateCell>
        </StateRow>
        <StateRow label="Invalid">
          <StateCell>
            <Field label="Verification code" error="That code has expired. Enter the new one from your app.">
              <OTPInput defaultValue="418207" />
            </Field>
          </StateCell>
        </StateRow>
        <StateRow label="Disabled">
          <StateCell id="otp-disabled">
            <OTPInput aria-label="Disabled" disabled />
          </StateCell>
        </StateRow>
        <StateRow label="Disabled, filled">
          <StateCell>
            <OTPInput aria-label="Disabled, filled" defaultValue="418207" disabled />
          </StateCell>
        </StateRow>
      </StateMatrix>
      <StateSection title="Sizes and widths">
        <div className="flex flex-col items-start gap-16">
          <OTPInput aria-label="Small" size="sm" defaultValue="41" data-force="focus" />
          <OTPInput aria-label="Four digits" length={4} defaultValue="0915" />
          <div className="w-240">
            <OTPInput aria-label="In a 240px column" defaultValue="418" />
          </div>
        </div>
      </StateSection>
    </>
  );
}
