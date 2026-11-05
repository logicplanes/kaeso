import { VisuallyHidden } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "VisuallyHidden",
  description: "Text for assistive technology only, such as the rest of a short label.",
  group: "Components",
  entry: "visually-hidden",
  import: "VisuallyHidden",
};
export const examples = [{ id: "basic", title: "VisuallyHidden", fn: "Basic" }];
export function Basic() {
  return (
    <button type="button" className="inline-flex h-36 items-center rounded-pill px-16 text-body font-medium material-secondary focus-ring">
      Export
      <VisuallyHidden> request logs as CSV</VisuallyHidden>
    </button>
  );
}
export function States() {
  return (
    <>
      <StateSection title="A button whose accessible name says more than its label">
        <div className="flex items-center gap-16">
          <Basic />
          <span className="text-small text-fg-secondary">Read as “Export request logs as CSV”</span>
        </div>
      </StateSection>
    </>
  );
}
