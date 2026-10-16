import { InputGroup, icons } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "InputGroup",
  description: "Addons inside the pill: text, icons, a ghost circle or a key cap, sharing one focus halo.",
  group: "Components",
  entry: "input",
  import: "InputGroup",
  a11y: ["Addons are text the input's label should not repeat; buttons inside need their own names."],
};
export const examples = [{ id: "addons", title: "Addons", fn: "Addons" }];
export function Addons() {
  return (
    <div className="flex w-400 flex-col gap-12">
      <InputGroup>
        <InputGroup.Addon>https://</InputGroup.Addon>
        <InputGroup.Input aria-label="Host" placeholder="gateway.northwind.example" />
        <InputGroup.Addon>/v1</InputGroup.Addon>
      </InputGroup>
      <InputGroup trailingButton>
        <InputGroup.Input aria-label="Token" defaultValue="sk-kso-AbC123…wxyz" className="mono text-small" />
        <InputGroup.Button icon={icons.copy} aria-label="Copy" />
      </InputGroup>
      <InputGroup leadingIcon>
        <InputGroup.Addon icon={icons.search} />
        <InputGroup.Input aria-label="Search" placeholder="Search models" />
      </InputGroup>
    </div>
  );
}
export function States() {
  return (
    <StateSection title="Focus shared by addons">
      <div className="flex w-400 flex-col gap-12">
        <Addons />
        <InputGroup data-force="focus">
          <InputGroup.Addon>USD</InputGroup.Addon>
          <InputGroup.Input aria-label="Budget" defaultValue="500.00" />
        </InputGroup>
        <InputGroup invalid>
          <InputGroup.Addon>USD</InputGroup.Addon>
          <InputGroup.Input aria-label="Budget" defaultValue="-1" aria-invalid />
        </InputGroup>
      </div>
    </StateSection>
  );
}
