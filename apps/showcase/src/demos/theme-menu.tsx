import { ThemeMenu, ThemeProvider } from "@logicplanes/kaeso-ui";
import { Matrix, StateSection } from "~/lib/states";

export const meta = {
  title: "ThemeMenu",
  description: "A circle that opens system, light or dark, stored as `kaeso:theme`; its icon shows the theme in use.",
  group: "Components",
  entry: "theme-menu",
  import: "ThemeMenu",
  keyboard: [
    ["Enter Space", "Opens the menu"],
    ["Up Down", "Moves between the choices"],
    ["Enter", "Chooses and closes"],
    ["Escape", "Closes and returns focus"],
  ],
  a11y: [
    "Named Theme in the provider's language; the menu marks the current choice as checked.",
    "Reads and sets the choice of the nearest ThemeProvider; every provider on the page follows it.",
  ],
};
export const examples = [
  { id: "basic", title: "Basic", fn: "Basic" },
  { id: "raised", title: "Raised", fn: "Raised" },
];
export function Basic() {
  return (
    <ThemeProvider>
      <ThemeMenu />
    </ThemeProvider>
  );
}
export function Raised() {
  return (
    <ThemeProvider>
      <ThemeMenu variant="secondary" size="md" />
    </ThemeProvider>
  );
}
export function States() {
  return (
    <ThemeProvider>
      <Matrix
        title="Variants × states"
        rows={[{ label: "ghost" }, { label: "secondary" }]}
        columns={[
          { label: "Rest", props: {} },
          { label: "Hover", props: { "data-force": "hover" } },
          { label: "Focus", props: { "data-force": "focus" } },
          { label: "Pressed", props: { "data-force": "pressed" } },
        ]}
        render={(r, p) => <ThemeMenu variant={r.label as "ghost"} {...p} />}
      />
      <StateSection title="Sizes">
        <div className="flex items-center gap-12">
          <ThemeMenu variant="secondary" size="sm" />
          <ThemeMenu variant="secondary" size="md" />
        </div>
      </StateSection>
    </ThemeProvider>
  );
}
