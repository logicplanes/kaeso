import { SearchInput } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";
import { FIELD_STATES, StateGrid } from "~/lib/state-grid";

export const meta = {
  title: "SearchInput",
  description: "Search with a leading icon, a clear circle when filled, and a shortcut hint where one was asked for.",
  group: "Components",
  entry: "input",
  import: "SearchInput",
  keyboard: [["Escape", "Clears the search"]],
  a11y: [
    "The clear button is named “Clear search”.",
    "type=search lets assistive technology announce it as a search field.",
    "A shortcut hint hides while the field has focus, where it no longer applies; its room stays, so nothing moves.",
  ],
};
export const examples = [
  { id: "toolbar", title: "In a toolbar", fn: "Toolbar" },
  { id: "settings", title: "In the settings sidebar", fn: "Settings" },
];
export function Toolbar() {
  return (
    <div className="w-240">
      <SearchInput aria-label="Search logs" placeholder="gpt-5, req_01…" shortcut={["mod", "k"]} />
    </div>
  );
}
export function Settings() {
  return (
    <div className="w-240">
      <SearchInput size="md" aria-label="Search settings" placeholder="Rate limit, budget" />
    </div>
  );
}
export function States() {
  return (
    <>
      <StateGrid
        title="States"
        kinds={[
          { label: "Empty, with shortcut", id: "empty" },
          { label: "Filled", id: "filled" },
        ]}
        states={FIELD_STATES.filter((s) => s.id !== "invalid")}
        render={(k, p) => (
          <div className="w-200">
            <SearchInput
              aria-label="Search logs"
              placeholder="gpt-5, req_01…"
              shortcut={["mod", "k"]}
              {...(k.id === "filled" ? { defaultValue: "gpt-5" } : {})}
              {...p}
            />
          </div>
        )}
      />
      <StateSection title="Sizes">
        <div className="flex w-240 flex-col gap-12">
          {(["sm", "md", "lg", "xl"] as const).map((s) => (
            <SearchInput key={s} size={s} aria-label={`Search, ${s}`} placeholder="gpt-5, req_01…" />
          ))}
        </div>
      </StateSection>
    </>
  );
}
