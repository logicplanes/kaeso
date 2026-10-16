import { VisuallyHidden, Portal, Surface } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "VisuallyHidden",
  description: "Utilities: text for assistive technology only, a portal to the body, and the internal Surface.",
  group: "Components",
  entry: "visually-hidden",
  import: "VisuallyHidden, Portal",
  a11y: ["Surface is internal: it draws only the framed surfaces of the foundations, never a generic card."],
};
export const examples = [
  { id: "basic", title: "VisuallyHidden", fn: "Basic" },
  { id: "surfaces", title: "Surface kinds", fn: "Surfaces" },
];
export function Basic() {
  return (
    <button type="button" className="inline-flex h-36 items-center rounded-pill px-16 text-body font-medium material-secondary focus-ring">
      Export
      <VisuallyHidden> request logs as CSV</VisuallyHidden>
    </button>
  );
}
export function Surfaces() {
  return (
    <div className="grid grid-cols-2 gap-16 sm:grid-cols-4">
      {(["panel", "menu", "group", "well"] as const).map((k) => (
        <Surface key={k} kind={k} className="flex h-80 w-140 items-center justify-center text-small text-fg-secondary">
          {k}
        </Surface>
      ))}
    </div>
  );
}
export function States() {
  return (
    <StateSection title="Surfaces">
      <Surfaces />
    </StateSection>
  );
}
void Portal;
