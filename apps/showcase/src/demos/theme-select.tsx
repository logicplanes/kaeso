import { Select, ThemeProvider, ThemeSelect } from "@logicplanes/kaeso-ui";
import { Matrix } from "~/lib/states";

export const meta = {
  title: "ThemeSelect",
  description: "A select for system, light or dark, stored as `kaeso:theme`, with its label in the provider's language.",
  group: "Components",
  entry: "theme-select",
  import: "ThemeSelect",
  keyboard: [
    ["Enter Space", "Opens"],
    ["Up Down", "Moves"],
    ["Type", "Jumps to an option"],
    ["Escape", "Closes and returns focus"],
  ],
  a11y: [
    "Named Theme in the provider's language, and shows the choice in words, so it needs no aria-label and no icon.",
    "Reads and sets the choice of the nearest ThemeProvider; every provider on the page follows it.",
  ],
};
export const examples = [
  { id: "basic", title: "Basic", fn: "Basic" },
  { id: "sidebar", title: "In a sidebar's footer, beside the language", fn: "SidebarFooter" },
  { id: "raised", title: "Raised", fn: "Raised" },
];
export function Basic() {
  return (
    <ThemeProvider>
      <ThemeSelect />
    </ThemeProvider>
  );
}
export function SidebarFooter() {
  return (
    <ThemeProvider>
      <div className="grid w-240 grid-cols-2 gap-4">
        <Select
          aria-label="Language"
          size="sm"
          appearance="ghost"
          fullWidth
          defaultValue="en"
          options={[
            { value: "en", label: "English" },
            { value: "de", label: "Deutsch" },
            { value: "zh-CN", label: "简体中文" },
          ]}
        />
        <ThemeSelect fullWidth />
      </div>
    </ThemeProvider>
  );
}
export function Raised() {
  return (
    <ThemeProvider>
      <ThemeSelect appearance="default" size="md" />
    </ThemeProvider>
  );
}
export function States() {
  return (
    <>
      <Matrix
        title="Trigger"
        rows={[{ label: "ghost" }, { label: "default" }]}
        columns={[
          { label: "Rest", props: {} },
          { label: "Hover", props: { "data-force": "hover" } },
          { label: "Focus", props: { "data-force": "focus" } },
          { label: "Open", props: { "data-force": "open" } },
        ]}
        render={(r, p) => <ThemeSelect appearance={r.label as "ghost"} {...p} />}
      />
      <Matrix
        title="Sizes"
        rows={(["sm", "md"] as const).map((s) => ({ label: s }))}
        columns={[{ label: "Value", props: {} }]}
        render={(r) => <ThemeSelect appearance="default" size={r.label as "sm"} />}
      />
    </>
  );
}
