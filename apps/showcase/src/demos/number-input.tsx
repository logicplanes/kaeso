import { useState } from "react";
import { Input, NumberInput } from "@logicplanes/kaeso-ui";
import { LocaleProvider } from "@logicplanes/kaeso-ui/i18n";
import { StateCell, StateMatrix, StateRow, StateSection } from "~/lib/states";
import { FIELD_STATES, StateGrid } from "~/lib/state-grid";

export const meta = {
  title: "NumberInput",
  description: "Exact numbers: end-aligned tabular digits, a unit, quiet steppers, a decimal string value.",
  group: "Components",
  entry: "input",
  import: "NumberInput",
  keyboard: [
    ["Up", "Increases by the step"],
    ["Down", "Decreases by the step"],
    ["Tab", "Moves past the field; the steppers are not tab stops, since the arrows do the same"],
  ],
  a11y: [
    "The value is a decimal string, so money never passes through a float.",
    "Thousands separators appear on blur and are removed while editing.",
    "The input is a spin button: screen readers hear its value with the unit, its limits, and that the arrow keys change it.",
    "A stepper turns off at the minimum or maximum.",
  ],
};
export const examples = [
  { id: "money", title: "Money", fn: "Money" },
  { id: "rate", title: "Rate", fn: "Rate" },
];
export function Money() {
  const [v, setV] = useState("1250.00");
  return (
    <div className="flex w-280 flex-col gap-8">
      <NumberInput label="Monthly budget" value={v} onValueChange={setV} unit="USD" step="50" min="0" />
      <span className="mono text-small text-fg-secondary">value = "{v}"</span>
    </div>
  );
}
export function Rate() {
  return (
    <div className="w-240">
      <NumberInput aria-label="Requests per minute" defaultValue="600" unit="/ min" step="10" />
    </div>
  );
}
const KINDS = [
  { label: "Money", id: "money" },
  { label: "Whole days", id: "days" },
];

export function States() {
  return (
    <>
      <StateGrid
        title="States × kinds"
        kinds={KINDS}
        states={FIELD_STATES}
        render={(k, p) => (
          <div className="w-200">
            {k.id === "money" ? (
              <NumberInput aria-label="Monthly budget" defaultValue="12500.50" unit="USD" {...p} />
            ) : (
              <NumberInput aria-label="Retention" integer defaultValue="90" unit="days" {...p} />
            )}
          </div>
        )}
      />
      <StateMatrix title="Limits, alignment and language" columns={["Value", "Note"]}>
        <StateRow label="At the minimum">
          <StateCell>
            <div className="w-200">
              <NumberInput aria-label="Budget at zero" defaultValue="0" min="0" unit="USD" data-force="hover" />
            </div>
          </StateCell>
          <StateCell>
            <span className="text-small text-fg-secondary">Decrease is off</span>
          </StateCell>
        </StateRow>
        <StateRow label="At the maximum">
          <StateCell>
            <div className="w-200">
              <NumberInput aria-label="Retention at the limit" integer defaultValue="365" max="365" unit="days" data-force="hover" />
            </div>
          </StateCell>
          <StateCell>
            <span className="text-small text-fg-secondary">Increase is off</span>
          </StateCell>
        </StateRow>
        <StateRow label="Start-aligned">
          <StateCell>
            <div className="w-200">
              <NumberInput aria-label="Requests per minute" align="start" defaultValue="600" unit="/ min" />
            </div>
          </StateCell>
          <StateCell>
            <span className="text-small text-fg-secondary">align="start"</span>
          </StateCell>
        </StateRow>
        <StateRow label="German">
          <StateCell>
            <LocaleProvider locale="de" messages={{}}>
              <div className="w-200">
                <NumberInput aria-label="Monatsbudget" defaultValue="1250.5" unit="EUR" />
              </div>
            </LocaleProvider>
          </StateCell>
          <StateCell>
            <span className="text-small text-fg-secondary">1.250,5</span>
          </StateCell>
        </StateRow>
      </StateMatrix>
      <StateSection title="Sizes, under a plain input">
        <div className="flex w-240 flex-col gap-12">
          <Input aria-label="Plain input" defaultValue="Ends on this edge" align="end" />
          {(["sm", "md", "lg", "xl"] as const).map((s) => (
            <NumberInput key={s} size={s} aria-label={`Size ${s}`} defaultValue="1200" unit="USD" />
          ))}
        </div>
      </StateSection>
    </>
  );
}
