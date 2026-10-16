import { Toolbar, FilterChip, Button, SearchInput, DisplayOptions, icons } from "@logicplanes/kaeso-ui";
import { useLogDisplayOptions } from "~/lib/display-options";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Toolbar",
  description: "Filters, search and display options on one 48px row, with arrow-key focus across them.",
  group: "Layout",
  entry: "toolbar",
  import: "Toolbar",
  keyboard: [["Left Right", "Moves between controls"]],
};
export const examples = [{ id: "logs", title: "Request logs", fn: "Logs" }];
export function Logs() {
  const display = useLogDisplayOptions();
  return (
    <div className="w-full rounded-well shadow-frame">
      <Toolbar aria-label="Filters">
        <FilterChip property="Status" operator="is" value="2xx" onRemove={() => undefined} />
        <FilterChip property="Model" operator="is" value="gpt-5" onRemove={() => undefined} />
        <Button variant="ghost" size="sm" leadingIcon={icons.add}>
          Add filter
        </Button>
        <Toolbar.Spacer />
        <div className="w-240">
          <SearchInput aria-label="Search logs" placeholder="Search logs" />
        </div>
        <DisplayOptions {...display} />
      </Toolbar>
    </div>
  );
}
export function States() {
  return (
    <StateSection title="Toolbar">
      <div className="w-960">
        <Logs />
      </div>
    </StateSection>
  );
}
