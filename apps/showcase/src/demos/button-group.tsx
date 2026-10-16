import { ButtonGroup, icons } from "@logicplanes/kaeso-ui";
import { Matrix, StateSection } from "~/lib/states";

export const meta = {
  title: "ButtonGroup",
  description: "Related actions side by side: connected pills, or spaced.",
  group: "Components",
  entry: "button-group",
  import: "ButtonGroup",
  keyboard: [["Tab", "Moves between the buttons"]],
  a11y: [
    "The group has role=group; give it an aria-label when the buttons alone do not explain it.",
    "Icon-only items need an aria-label and show it as a tooltip.",
  ],
};
export const examples = [
  { id: "attached", title: "Connected", fn: "Attached" },
  { id: "spaced", title: "Spaced", fn: "Spaced" },
  { id: "icons", title: "Icons", fn: "Icons" },
];
export function Attached() {
  return (
    <ButtonGroup aria-label="Export">
      <ButtonGroup.Item>CSV</ButtonGroup.Item>
      <ButtonGroup.Item>JSON</ButtonGroup.Item>
      <ButtonGroup.Item>NDJSON</ButtonGroup.Item>
    </ButtonGroup>
  );
}
export function Spaced() {
  return (
    <ButtonGroup attached={false} aria-label="Key actions">
      <ButtonGroup.Item>Disable</ButtonGroup.Item>
      <ButtonGroup.Item>Rotate</ButtonGroup.Item>
    </ButtonGroup>
  );
}
export function Icons() {
  return (
    <ButtonGroup size="sm" aria-label="Zoom">
      <ButtonGroup.Item icon={icons.minus} aria-label="Zoom out" />
      <ButtonGroup.Item>100%</ButtonGroup.Item>
      <ButtonGroup.Item icon={icons.add} aria-label="Zoom in" />
    </ButtonGroup>
  );
}
export function States() {
  return (
    <>
      <Matrix
        title="Item states (middle item)"
        rows={[{ label: "attached" }, { label: "spaced" }]}
        columns={[
          { label: "Rest", props: {} },
          { label: "Hover", props: { "data-force": "hover" } },
          { label: "Focus", props: { "data-force": "focus" } },
          { label: "Pressed", props: { "data-force": "pressed" } },
          { label: "Selected", props: { pressed: true } },
          { label: "Disabled", props: { disabled: true } },
        ]}
        render={(r, p) => (
          <ButtonGroup size="sm" attached={r.label === "attached"}>
            <ButtonGroup.Item>Day</ButtonGroup.Item>
            <ButtonGroup.Item {...p}>Week</ButtonGroup.Item>
            <ButtonGroup.Item>Month</ButtonGroup.Item>
          </ButtonGroup>
        )}
      />
      <StateSection title="Sizes">
        <div className="flex flex-col items-start gap-12">
          {(["xs", "sm", "md", "lg"] as const).map((s) => (
            <ButtonGroup key={s} size={s}>
              <ButtonGroup.Item>Day</ButtonGroup.Item>
              <ButtonGroup.Item>Week</ButtonGroup.Item>
              <ButtonGroup.Item icon={icons.calendar} aria-label="Custom range" />
            </ButtonGroup>
          ))}
        </div>
      </StateSection>
    </>
  );
}
