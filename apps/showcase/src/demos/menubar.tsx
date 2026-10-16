import { Menubar, icons } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Menubar",
  description: "A row of ghost pills, each opening a menu; arrows move between them.",
  group: "Components",
  entry: "menu",
  import: "Menubar",
  keyboard: [
    ["Left Right", "Moves between menus"],
    ["Down", "Opens the menu"],
  ],
};
export const examples = [{ id: "basic", title: "Basic", fn: "Basic" }];
export function Basic() {
  return (
    <Menubar aria-label="Request">
      <Menubar.Menu>
        <Menubar.Trigger>Request</Menubar.Trigger>
        <Menubar.Content>
          <Menubar.Item icon={icons.copy}>Copy as curl</Menubar.Item>
          <Menubar.Item icon={icons.download}>Download JSON</Menubar.Item>
        </Menubar.Content>
      </Menubar.Menu>
      <Menubar.Menu>
        <Menubar.Trigger>View</Menubar.Trigger>
        <Menubar.Content>
          <Menubar.CheckboxItem defaultChecked>Conversation</Menubar.CheckboxItem>
          <Menubar.CheckboxItem>Raw JSON</Menubar.CheckboxItem>
        </Menubar.Content>
      </Menubar.Menu>
    </Menubar>
  );
}
export function States() {
  return (
    <StateSection title="Triggers">
      <Menubar aria-label="States">
        <Menubar.Menu>
          <Menubar.Trigger>Rest</Menubar.Trigger>
        </Menubar.Menu>
        <Menubar.Menu>
          <Menubar.Trigger data-force="hover">Hover</Menubar.Trigger>
        </Menubar.Menu>
        <Menubar.Menu>
          <Menubar.Trigger data-force="open">Open</Menubar.Trigger>
        </Menubar.Menu>
        <Menubar.Menu>
          <Menubar.Trigger data-force="focus">Focus</Menubar.Trigger>
        </Menubar.Menu>
      </Menubar>
    </StateSection>
  );
}
