import { Segmented, icons } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

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
export function States() {
  return (
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
  );
}
