import { Button, Popover, HoverCard, Input, KeyValue, ModelLabel } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Popover",
  description: "A floating panel next to its trigger; HoverCard previews on hover, never as the only way in.",
  group: "Components",
  entry: "popover",
  import: "Popover, HoverCard",
  keyboard: [["Escape", "Closes and returns focus"]],
  a11y: ["Popovers with a form trap focus (trapFocus); others do not."],
};
export const examples = [
  { id: "form", title: "With a form", fn: "Form" },
  { id: "hover", title: "HoverCard", fn: "Hover" },
];
export function Form() {
  return (
    <Popover trapFocus>
      <Popover.Trigger>
        <Button>Rename</Button>
      </Popover.Trigger>
      <Popover.Content title="Rename key" className="w-320">
        <div className="flex flex-col gap-12">
          <Input aria-label="Name" defaultValue="checkout-web" />
          <div className="flex justify-end gap-8">
            <Popover.Close>
              <Button size="sm">Cancel</Button>
            </Popover.Close>
            <Button size="sm" variant="primary">
              Save
            </Button>
          </div>
        </div>
      </Popover.Content>
    </Popover>
  );
}
export function Hover() {
  return (
    <HoverCard>
      <HoverCard.Trigger>
        <a href="#" className="mono text-small text-fg underline decoration-line-strong underline-offset-3">
          claude-sonnet-4-5
        </a>
      </HoverCard.Trigger>
      <HoverCard.Content>
        <div className="flex flex-col gap-12">
          <ModelLabel provider="anthropic" providerName="Anthropic" model="claude-sonnet-4-5" capabilities={["tools", "reasoning", "vision", "pdf"]} />
          <KeyValue
            items={[
              { label: "Input", value: "$3.00 / M" },
              { label: "Output", value: "$15.00 / M" },
              { label: "Context", value: "200,000 tokens" },
            ]}
          />
        </div>
      </HoverCard.Content>
    </HoverCard>
  );
}
export function States() {
  return (
    <StateSection title="Open">
      <div className="h-240 w-400">
        <Popover open>
          <Popover.Trigger>
            <Button data-force="open">Rename</Button>
          </Popover.Trigger>
          <Popover.Content title="Rename key" className="w-320">
            <Input aria-label="Name" defaultValue="checkout-web" />
          </Popover.Content>
        </Popover>
      </div>
    </StateSection>
  );
}
