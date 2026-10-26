import { Segmented, icons } from "@logicplanes/kaeso-ui";
import { Matrix, StateSection } from "~/lib/states";

export const meta = {
  title: "Segmented",
  description: "Two to five exclusive options on a fill track; the active one raised, sliding in 180ms.",
  group: "Components",
  entry: "segmented",
  import: "Segmented",
  keyboard: [["Left Right", "Moves and selects"]],
  a11y: ["A radio group: one stop in the tab order, arrows choose.", "Icon-only options need an aria-label, shown as a tooltip."],
};
export const examples = [
  { id: "period", title: "Period", fn: "Period" },
  { id: "view", title: "View with icons", fn: "View" },
];
export function Period() {
  return (
    <Segmented
      aria-label="Period"
      defaultValue="7d"
      options={[
        { value: "24h", label: "24h" },
        { value: "7d", label: "7d" },
        { value: "30d", label: "30d" },
        { value: "90d", label: "90d" },
      ]}
    />
  );
}
export function View() {
  return (
    <Segmented
      aria-label="View"
      size="sm"
      defaultValue="list"
      options={[
        { value: "list", label: "List", icon: icons.list },
        { value: "board", label: "Board", icon: icons.board },
      ]}
    />
  );
}
const period = [
  { value: "7d", label: "7d" },
  { value: "30d", label: "30d" },
];

function PeriodWith({ target, props }: { target: string; props: Record<string, unknown> }) {
  const { disabled, ...force } = props as { disabled?: boolean; "data-force"?: string };
  return (
    <Segmented
      aria-label="Period"
      size="sm"
      defaultValue="7d"
      options={period.map((o) => (o.value === target ? { ...o, ...force, ...(disabled ? { disabled } : {}) } : o))}
    />
  );
}

export function States() {
  return (
    <>
      <Matrix
        title="Options"
        rows={[{ label: "unselected" }]}
        columns={[
          { label: "Rest", props: {} },
          { label: "Hover", props: { "data-force": "hover" } },
          { label: "Pressed", props: { "data-force": "pressed" } },
        ]}
        render={(_, p) => <PeriodWith target="30d" props={p} />}
      />
      <Matrix
        title="Keyboard and off"
        rows={[{ label: "unselected" }]}
        columns={[
          { label: "Focus", props: { "data-force": "focus" } },
          { label: "Disabled", props: { disabled: true } },
        ]}
        render={(_, p) => <PeriodWith target="30d" props={p} />}
      />
      <Matrix
        title="Selected option"
        rows={[{ label: "selected" }]}
        columns={[
          { label: "Rest", props: {} },
          { label: "Focus", props: { "data-force": "focus" } },
          { label: "Disabled", props: { disabled: true } },
        ]}
        render={(_, p) => <PeriodWith target="7d" props={p} />}
      />
      <StateSection title="Sizes and content">
        <div className="flex flex-col items-start gap-16">
          <Period />
          <View />
          <Segmented
            aria-label="Icons only"
            size="sm"
            defaultValue="list"
            options={[
              { value: "list", icon: icons.list, "aria-label": "List" },
              { value: "board", icon: icons.board, "aria-label": "Board" },
            ]}
          />
          <Segmented
            aria-label="Disabled"
            disabled
            defaultValue="a"
            options={[
              { value: "a", label: "OpenAI" },
              { value: "b", label: "Anthropic" },
            ]}
          />
          <Segmented
            aria-label="Retention"
            defaultValue="30"
            options={[
              { value: "7", label: "7 Tage" },
              { value: "30", label: "30 Tage" },
              { value: "unlimited", label: "Unbegrenzt aufbewahren" },
            ]}
          />
          <div className="w-400">
            <Segmented
              aria-label="Full width"
              fullWidth
              defaultValue="a"
              options={[
                { value: "a", label: "Responses" },
                { value: "b", label: "Chat Completions" },
              ]}
            />
          </div>
        </div>
      </StateSection>
    </>
  );
}
