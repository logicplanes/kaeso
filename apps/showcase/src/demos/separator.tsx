import { Separator } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Separator",
  description: "A 1px line, for when space and tone cannot separate two regions.",
  group: "Components",
  entry: "separator",
  import: "Separator",
  a11y: ["Renders role=separator with its orientation."],
};
export const examples = [
  { id: "basic", title: "Basic", fn: "Basic" },
  { id: "label", title: "With a label", fn: "Labelled" },
];
export function Basic() {
  return (
    <div className="flex w-320 flex-col gap-12 text-body text-fg">
      <span>Above</span>
      <Separator />
      <span>Below</span>
    </div>
  );
}
export function Labelled() {
  return (
    <div className="w-320">
      <Separator label="or" />
    </div>
  );
}
export function States() {
  return (
    <StateSection title="Tones and orientation">
      <div className="flex w-400 flex-col gap-16">
        <Separator />
        <Separator tone="default" />
        <Separator label="or continue with" />
        <div className="flex h-40 items-center gap-12 text-body">
          <span>Left</span>
          <Separator orientation="vertical" />
          <span>Right</span>
        </div>
      </div>
    </StateSection>
  );
}
