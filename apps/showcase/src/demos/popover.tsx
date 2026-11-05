import { Button, Popover, Input } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Popover",
  description: "A floating panel next to its trigger, opened by a click and closed with Escape.",
  group: "Components",
  entry: "popover",
  import: "Popover",
  keyboard: [["Escape", "Closes and returns focus"]],
  a11y: [
    "Popovers with a form trap focus (trapFocus); others do not.",
    "The title names the popover's dialog; without a title, give the content an aria-label.",
    "A popover is never wider than the screen.",
  ],
};
export const examples = [{ id: "form", title: "With a form", fn: "Form" }];
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
export function States() {
  return (
    <StateSection title="Open: a form, and a list without a title">
      <div className="flex flex-col gap-24">
        <div className="h-160 w-400">
          <Popover open>
            <Popover.Trigger>
              <Button data-force="open">Rename</Button>
            </Popover.Trigger>
            <Popover.Content title="Rename key" className="w-320">
              <Input aria-label="Name" defaultValue="checkout-web" />
            </Popover.Content>
          </Popover>
        </div>
        <div className="h-200 w-400">
          <Popover open>
            <Popover.Trigger>
              <Button data-force="open">Recent keys</Button>
            </Popover.Trigger>
            <Popover.Content aria-label="Recent keys" padding="list" className="w-240">
              <div className="flex flex-col">
                {["checkout-web", "support-bot", "nightly-evals"].map((k) => (
                  <span key={k} className="flex h-34 items-center rounded-item px-10 mono text-small text-fg">
                    {k}
                  </span>
                ))}
              </div>
            </Popover.Content>
          </Popover>
        </div>
      </div>
    </StateSection>
  );
}
