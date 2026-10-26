import { Menubar, icons } from "@logicplanes/kaeso-ui";
import { StateCell, StateMatrix, StateRow, StateSection } from "~/lib/states";

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
const TRIGGERS = [
  ["rest", {}],
  ["hover", { "data-force": "hover" }],
  ["focus", { "data-force": "focus" }],
  ["pressed", { "data-force": "pressed" }],
  ["open", { "data-force": "open" }],
] as const;
export function States() {
  return (
    <>
      <StateMatrix title="Triggers" columns={TRIGGERS.map(([s]) => s[0]!.toUpperCase() + s.slice(1))}>
        <StateRow label="Trigger">
          {TRIGGERS.map(([s, p]) => (
            <StateCell key={s} id={`trigger-${s}`}>
              <Menubar aria-label="Request">
                <Menubar.Menu>
                  <Menubar.Trigger {...p}>Request</Menubar.Trigger>
                </Menubar.Menu>
              </Menubar>
            </StateCell>
          ))}
        </StateRow>
      </StateMatrix>
      <StateSection title="Side by side, focused, and an open menu">
        <div className="flex h-160 flex-col gap-24">
          <Menubar aria-label="Request">
            <Menubar.Menu>
              <Menubar.Trigger data-force="focus">Request</Menubar.Trigger>
            </Menubar.Menu>
            <Menubar.Menu>
              <Menubar.Trigger data-force="hover">View</Menubar.Trigger>
            </Menubar.Menu>
            <Menubar.Menu open>
              <Menubar.Trigger>Export</Menubar.Trigger>
              <Menubar.Content>
                <Menubar.Item icon={icons.copy}>Copy as curl</Menubar.Item>
                <Menubar.Item icon={icons.download}>Download JSON</Menubar.Item>
              </Menubar.Content>
            </Menubar.Menu>
          </Menubar>
        </div>
      </StateSection>
    </>
  );
}
