import { Toolbar, FilterChip, Button, SearchInput, DisplayOptions, icons } from "@logicplanes/kaeso-ui";
import { useLogDisplayOptions } from "~/lib/display-options";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Toolbar",
  description: "Filters, search and display options on one 48px row, with arrow-key focus across them.",
  group: "Layout",
  entry: "toolbar",
  import: "Toolbar",
  keyboard: [
    ["Tab", "Enters the toolbar on the control used last, and leaves it"],
    ["Left Right", "Moves between controls, wrapping at the ends"],
  ],
  a11y: ["One tab stop: arrows move inside, skipping the parts of a filter chip that aren't controls; a filled search keeps the arrows for its caret."],
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
        <div className="order-last basis-full sm:order-none sm:basis-auto sm:w-240">
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
      <div className="w-640">
        <Logs />
      </div>
    </StateSection>
  );
}
