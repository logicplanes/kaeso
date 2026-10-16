import { ContextMenu, icons } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "ContextMenu",
  description: "The same menu on right-click and the context-menu key.",
  group: "Components",
  entry: "menu",
  import: "ContextMenu",
  keyboard: [["Shift F10", "Opens the menu for the focused element"]],
  a11y: ["Every action in a context menu must also be reachable another way."],
};
export const examples = [{ id: "row", title: "On a row", fn: "Row" }];
export function Row() {
  return (
    <ContextMenu>
      <ContextMenu.Trigger className="flex h-44 w-400 items-center rounded-item bg-fill-1 px-16 text-body text-fg">
        Right-click this request
      </ContextMenu.Trigger>
      <ContextMenu.Content>
        <ContextMenu.Item icon={icons.openExternal}>Open in a sheet</ContextMenu.Item>
        <ContextMenu.Item icon={icons.copy}>Copy request id</ContextMenu.Item>
        <ContextMenu.Item icon={icons.filter}>Filter by this model</ContextMenu.Item>
      </ContextMenu.Content>
    </ContextMenu>
  );
}
export function States() {
  return (
    <StateSection title="Trigger">
      <Row />
    </StateSection>
  );
}
