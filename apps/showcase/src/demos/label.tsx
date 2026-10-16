import { Label, Input } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Label",
  description: "The name of a field: 14/500, with an optional “Optional”.",
  group: "Components",
  entry: "label",
  import: "Label",
  a11y: ["Connect it to its control with htmlFor; Field does this for you."],
};
export const examples = [
  { id: "basic", title: "Basic", fn: "Basic" },
  { id: "optional", title: "Optional", fn: "Optional" },
];
export function Basic() {
  return (
    <div className="flex w-320 flex-col gap-8">
      <Label htmlFor="name">Key name</Label>
      <Input id="name" placeholder="checkout-web" />
    </div>
  );
}
export function Optional() {
  return <Label optional>Description</Label>;
}
export function States() {
  return (
    <StateSection title="Sizes">
      <div className="flex flex-col gap-12">
        <Label>Key name</Label>
        <Label size="sm">Key name</Label>
        <Label optional>Description</Label>
      </div>
    </StateSection>
  );
}
