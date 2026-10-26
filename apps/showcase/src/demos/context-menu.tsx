import { Button, ContextMenu, Menu, icons } from "@logicplanes/kaeso-ui";
import { StateCell, StateMatrix, StateRow, StateSection } from "~/lib/states";

export const meta = {
  title: "ContextMenu",
  description: "The same menu on right-click and the context-menu key.",
  group: "Components",
  entry: "menu",
  import: "ContextMenu",
  keyboard: [
    ["Shift F10", "Opens the menu for the focused row"],
    ["Escape", "Closes and returns focus to the row"],
  ],
  a11y: [
    "Every action in a context menu must also be reachable another way.",
    "An area with nothing focusable in it becomes a focus stop, so the keyboard can open its menu.",
  ],
};
export const examples = [{ id: "row", title: "On a row", fn: "Row" }];
const ROW = "flex h-44 w-400 items-center rounded-item bg-fill-1 px-16 text-body text-fg";
export function Row() {
  return (
    <ContextMenu>
      <ContextMenu.Trigger className={ROW}>Right-click this request</ContextMenu.Trigger>
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
    <>
      <StateMatrix title="Trigger" columns={["Area"]}>
        <StateRow label="Rest">
          <StateCell id="area-rest">
            <Row />
          </StateCell>
        </StateRow>
        <StateRow label="Focus">
          <StateCell id="area-focus">
            <ContextMenu>
              <ContextMenu.Trigger className={ROW} data-force="focus">
                Right-click this request
              </ContextMenu.Trigger>
              <ContextMenu.Content>
                <ContextMenu.Item>Copy request id</ContextMenu.Item>
              </ContextMenu.Content>
            </ContextMenu>
          </StateCell>
        </StateRow>
      </StateMatrix>
      <StateSection title="The menu it opens, with a header">
        <div className="h-240">
          <Menu open>
            <Menu.Trigger>
              <Button data-force="open">req_01JAR4X2KQ</Button>
            </Menu.Trigger>
            <Menu.Content header={<span className="mono text-small text-fg-secondary">req_01JAR4X2KQ</span>}>
              <Menu.Item icon={icons.openExternal}>Open in a sheet</Menu.Item>
              <Menu.Item icon={icons.copy} data-force="highlighted">
                Copy request id
              </Menu.Item>
              <Menu.Item icon={icons.filter}>Filter by this model</Menu.Item>
            </Menu.Content>
          </Menu>
        </div>
      </StateSection>
    </>
  );
}
