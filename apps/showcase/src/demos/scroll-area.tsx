import { ScrollArea } from "@logicplanes/kaeso-ui";
import { models } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "ScrollArea",
  description: "A scroll container with thin scrollbars that appear on hover.",
  group: "Components",
  entry: "scroll-area",
  import: "ScrollArea",
  a11y: ["Give a focusable scroll region an aria-label so keyboard users know what they scroll."],
};
export const examples = [{ id: "basic", title: "Basic", fn: "Basic" }];
export function Basic() {
  return (
    <ScrollArea className="h-160 w-320 rounded-well bg-fill-1" aria-label="Models">
      <ul className="flex flex-col p-12">
        {[...models, ...models, ...models].map((m, i) => (
          <li key={i} className="flex h-32 items-center mono text-small text-fg">
            {m.model}
          </li>
        ))}
      </ul>
    </ScrollArea>
  );
}
export function States() {
  return (
    <StateSection title="Vertical">
      <Basic />
    </StateSection>
  );
}
