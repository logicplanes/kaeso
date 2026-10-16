import { SearchInput } from "@logicplanes/kaeso-ui";
import { Matrix } from "~/lib/states";

export const meta = {
  title: "SearchInput",
  description: "Search with a leading icon, a clear circle when filled, and a shortcut hint where one was asked for.",
  group: "Components",
  entry: "input",
  import: "SearchInput",
  keyboard: [["Escape", "Clears the search"]],
  a11y: ["The clear button is named “Clear search”.", "type=search lets assistive technology announce it as a search field."],
};
export const examples = [
  { id: "toolbar", title: "In a toolbar", fn: "Toolbar" },
  { id: "settings", title: "In the settings sidebar", fn: "Settings" },
];
export function Toolbar() {
  return (
    <div className="w-240">
      <SearchInput aria-label="Search logs" placeholder="Search logs" />
    </div>
  );
}
export function Settings() {
  return (
    <div className="w-240">
      <SearchInput size="md" aria-label="Search settings" placeholder="Search…" />
    </div>
  );
}
export function States() {
  return (
    <Matrix
      title="States"
      rows={[{ label: "sm" }, { label: "md" }]}
      columns={[
        { label: "Empty", props: { shortcut: ["mod", "k"] } },
        { label: "Hover", props: { "data-force": "hover" } },
        { label: "Focus", props: { "data-force": "focus" } },
        { label: "Filled", props: { defaultValue: "gpt-5" } },
        { label: "Disabled", props: { disabled: true } },
      ]}
      render={(r, p) => (
        <div className="w-200">
          <SearchInput aria-label="Search" size={r.label as "sm"} {...p} />
        </div>
      )}
    />
  );
}
