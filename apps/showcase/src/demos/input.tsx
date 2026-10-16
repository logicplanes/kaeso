import { Input, icons, type InputSize } from "@logicplanes/kaeso-ui";
import { Matrix, StateSection } from "~/lib/states";

export const meta = {
  title: "Input",
  description: "A pill text field with the secondary material and a soft focus halo.",
  group: "Components",
  entry: "input",
  import: "Input",
  keyboard: [["Tab", "Moves focus in and out"]],
  a11y: [
    "Pass label and description and the input wraps itself in Field, wiring aria-describedby; invalid sets aria-invalid. Errors are toasts, never text under the field.",
    "Placeholders are hints, never labels.",
    "Read-only fields stay selectable and focusable.",
  ],
};
export const examples = [
  { id: "basic", title: "Basic", fn: "Basic" },
  { id: "field", title: "With label and help", fn: "WithField", description: "An invalid value gets the danger ring; what is wrong goes in a toast." },
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
      <Input label="Monthly budget" defaultValue="-20" suffix="USD" invalid />
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
      <Input aria-label="Gateway URL" prefix="https://" placeholder="gateway.northwind.example" />
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
export function States() {
  return (
    <>
      <Matrix
        title="States"
        rows={[{ label: "empty" }, { label: "filled" }]}
        columns={[
          { label: "Rest", props: {} },
          { label: "Hover", props: { "data-force": "hover" } },
          { label: "Focus", props: { "data-force": "focus" } },
          { label: "Invalid", props: { invalid: true } },
          { label: "Disabled", props: { disabled: true } },
          { label: "Read-only", props: { readOnly: true } },
        ]}
        render={(r, p) => (
          <div className="w-200">
            <Input aria-label="Key name" placeholder="checkout-web" {...(r.label === "filled" ? { defaultValue: "checkout-web" } : {})} {...p} />
          </div>
        )}
      />
      <Matrix
        title="Sizes × content"
        rows={(["sm", "md", "lg", "xl"] as InputSize[]).map((s) => ({ label: s }))}
        columns={[
          { label: "Text", props: {} },
          { label: "Icon", props: { leadingIcon: icons.search } },
          { label: "Prefix", props: { prefix: "https://" } },
          { label: "Suffix", props: { suffix: "USD", align: "end" } },
        ]}
        render={(r, p) => (
          <div className="w-200">
            <Input size={r.label as InputSize} aria-label="Value" defaultValue="1200" {...p} />
          </div>
        )}
      />
      <StateSection title="Field">
        <div className="flex w-640 flex-col gap-24">
          <Input label="Key name" description="Shown in logs and the audit log." defaultValue="checkout-web" />
          <Input label="Monthly budget" defaultValue="-20" suffix="USD" invalid />
          <Input label="Description" optional placeholder="What this key is for" />
          <Input label="Rate limit" layout="horizontal" description="Requests per minute for this key." defaultValue="600" suffix="/ min" />
        </div>
      </StateSection>
    </>
  );
}
