import { useState } from "react";
import { Button, CommandPalette, icons, type CommandGroup } from "@logicplanes/kaeso-ui";
import { models, projects } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

const groups: CommandGroup[] = [
  {
    label: "Go to",
    items: [
      { id: "logs", label: "Request logs", icon: icons.requestLog },
      { id: "usage", label: "Usage", icon: icons.usage },
      { id: "keys", label: "Keys", icon: icons.key },
      {
        id: "projects",
        label: "Switch project",
        icon: icons.project,
        children: [{ label: "Projects", items: projects.map((p) => ({ id: p.id, label: p.name, icon: icons.project, hint: p.id })) }],
      },
    ],
  },
  { label: "Models", items: models.map((m) => ({ id: m.id, label: m.model, hint: m.name, icon: icons.model })) },
  {
    label: "Actions",
    items: [
      { id: "new-key", label: "Create key", icon: icons.add },
      { id: "theme", label: "Change theme", icon: icons.themeDark },
    ],
  },
];

export const meta = {
  title: "CommandPalette",
  description: "⌘K: every page and action, grouped; nested pages; async groups with a quiet progress line.",
  group: "Components",
  entry: "command-palette",
  import: "CommandPalette",
  keyboard: [
    ["mod K", "Opens"],
    ["Up Down", "Moves"],
    ["Enter", "Opens the item or its page"],
    ["Backspace", "Goes back a page when the search is empty"],
    ["Escape", "Closes"],
  ],
  a11y: ["The input is a combobox controlling the results listbox; the active result is announced."],
};
export const examples = [{ id: "basic", title: "Basic", fn: "Basic" }];
export function Basic() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button leadingIcon={icons.search} onClick={() => setOpen(true)} size="lg">
        Search
      </Button>
      <CommandPalette open={open} onOpenChange={setOpen} groups={groups} hotkey={false} />
    </>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Results">
        <CommandPalette inline groups={groups} />
      </StateSection>
      <StateSection title="Filtered and loading">
        <div className="flex flex-col gap-24">
          <CommandPalette inline groups={groups} defaultQuery="claude" />
          <CommandPalette inline groups={[{ label: "Logs", items: [], loading: true }]} defaultQuery="req_01" />
          <CommandPalette inline groups={groups} defaultQuery="zzz" emptyText="No pages or actions match" />
        </div>
      </StateSection>
      <StateSection title="Key hints, where they were asked for">
        <CommandPalette
          inline
          keyHints
          groups={[
            {
              label: "Go to",
              items: [
                { id: "logs", label: "Request logs", icon: icons.requestLog, shortcut: [["g"], ["l"]] },
                { id: "usage", label: "Usage", icon: icons.usage },
              ],
            },
          ]}
        />
      </StateSection>
    </>
  );
}
