import { Input, icons, type InputSize } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";
import { FIELD_STATES, StateGrid } from "~/lib/state-grid";

export const meta = {
  title: "Input",
  description: "A pill text field with the secondary material and a soft focus halo.",
  group: "Components",
  entry: "input",
  import: "Input",
  keyboard: [["Tab", "Moves focus in and out"]],
  a11y: [
    "Pass label and description and the input wraps itself in Field, wiring aria-describedby; invalid sets aria-invalid, and an error shows under the input.",
    "Placeholders are hints, never labels.",
    "Read-only fields stay selectable and focusable.",
  ],
};
export const examples = [
  { id: "basic", title: "Basic", fn: "Basic" },
  {
    id: "field",
    title: "With label and help",
    fn: "WithField",
    description: "An invalid value gets the danger ring and a message under the field that says what to enter.",
  },
  { id: "sizes", title: "Sizes", fn: "Sizes" },
  { id: "addons", title: "Icon, prefix and suffix", fn: "Addons" },
  { id: "readonly", title: "Read-only and disabled", fn: "ReadOnly" },
];
export function Basic() {
  return (
    <div className="w-320">
      <Input aria-label="Key name" placeholder="checkout-web" />
    </div>
  );
}
export function WithField() {
  return (
    <div className="flex w-360 flex-col gap-24">
      <Input label="Key name" description="Shown in logs and the audit log." placeholder="checkout-web" />
      <Input label="Monthly budget" defaultValue="-20" suffix="USD" align="end" error="Enter an amount of 0 or more." />
    </div>
  );
}
export function Sizes() {
  return (
    <div className="flex w-320 flex-col gap-12">
      {(["sm", "md", "lg", "xl"] as InputSize[]).map((s) => (
        <Input key={s} size={s} aria-label={`Size ${s}`} placeholder={`Size ${s}`} />
      ))}
    </div>
  );
}
export function Addons() {
  return (
    <div className="flex w-360 flex-col gap-12">
      <Input aria-label="Email" leadingIcon={icons.mail} placeholder="ada@northwind.example" />
      <Input aria-label="Gateway URL" prefix="https://" placeholder="kaeso.example.com" />
      <Input aria-label="Rate limit" suffix="/ min" defaultValue="600" align="end" />
    </div>
  );
}
export function ReadOnly() {
  return (
    <div className="flex w-320 flex-col gap-12">
      <Input aria-label="Project id" readOnly defaultValue="proj_01J9Z5A" />
      <Input aria-label="Disabled" disabled defaultValue="Managed by the organization" />
    </div>
  );
}
const INPUT_KINDS = [
  { label: "Empty", id: "empty" },
  { label: "Filled", id: "filled" },
];

export function States() {
  return (
    <>
      <StateGrid
        title="States"
        kinds={INPUT_KINDS}
        states={FIELD_STATES}
        render={(k, p) => (
          <div className="w-200">
            <Input aria-label="Key name" placeholder="checkout-web" {...(k.id === "filled" ? { defaultValue: "checkout-web" } : {})} {...p} />
          </div>
        )}
      />
      <StateGrid
        title="Content × states"
        kinds={[
          { label: "Prefix", id: "prefix" },
          { label: "Icon and suffix", id: "suffix" },
        ]}
        states={FIELD_STATES.filter((s) => ["rest", "focus", "disabled", "read-only"].includes(s.id))}
        render={(k, p) => (
          <div className="w-200">
            {k.id === "prefix" ? (
              <Input aria-label="Gateway URL" prefix="https://" defaultValue="kaeso.example.com" {...p} />
            ) : (
              <Input aria-label="Budget" leadingIcon={icons.budget} suffix="USD" align="end" defaultValue="1,200.00" {...p} />
            )}
          </div>
        )}
      />
      <StateSection title="Sizes">
        <div className="flex w-240 flex-col gap-12">
          {(["sm", "md", "lg", "xl"] as InputSize[]).map((s) => (
            <Input key={s} size={s} aria-label={`Size ${s}`} prefix="https://" defaultValue="kaeso.example.com" />
          ))}
        </div>
      </StateSection>
      <StateSection title="Field">
        <div className="flex w-560 flex-col gap-24">
          <Input label="Key name" description="Shown in logs and the audit log." defaultValue="checkout-web" />
          <Input
            label="Monthly budget"
            description="Requests stop once this is spent."
            error="Enter an amount of 0 or more."
            defaultValue="-20"
            suffix="USD"
            align="end"
          />
          <Input label="Description" optional placeholder="What this key is for" />
          <Input label="Rate limit" layout="horizontal" description="Requests per minute for this key." defaultValue="600" suffix="/ min" />
          <Input label="Origin" defaultValue="https://checkout.northwind.example/api/v2/callbacks/payment-confirmation" />
        </div>
      </StateSection>
    </>
  );
}
