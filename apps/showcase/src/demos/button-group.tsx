import { useState } from "react";
import { ButtonGroup, icons } from "@logicplanes/kaeso-ui";
import { StateCell, StateMatrix, StateRow, StateSection } from "~/lib/states";

export const meta = {
  title: "ButtonGroup",
  description: "Related actions side by side: connected pills, or spaced.",
  group: "Components",
  entry: "button-group",
  import: "ButtonGroup",
  keyboard: [
    ["Tab", "Moves between the buttons"],
    ["Enter", "Activates the focused button"],
    ["Space", "Activates the focused button"],
  ],
  a11y: [
    "The group has role=group; give it an aria-label when the buttons alone do not explain it.",
    "Icon-only items need an aria-label and show it as a tooltip.",
    "Items with pressed are toggles: aria-pressed says which is selected, and the selected item turns ink, well above 3:1 in both themes, so the choice never rests on a faint fill.",
  ],
};
export const examples = [
  { id: "attached", title: "Connected", fn: "Attached" },
  { id: "spaced", title: "Spaced", fn: "Spaced" },
  { id: "icons", title: "Icons", fn: "Icons" },
  {
    id: "toggle",
    title: "Toggle",
    fn: "Toggle",
    description:
      "Pass pressed to make items toggles, such as display options in a log view; selected items turn ink. Choosing one view among a few is Segmented.",
  },
];
export function Attached() {
  return (
    <ButtonGroup aria-label="Export">
      <ButtonGroup.Item>CSV</ButtonGroup.Item>
      <ButtonGroup.Item>JSON</ButtonGroup.Item>
      <ButtonGroup.Item>NDJSON</ButtonGroup.Item>
    </ButtonGroup>
  );
}
export function Spaced() {
  return (
    <ButtonGroup attached={false} aria-label="Key actions">
      <ButtonGroup.Item>Disable</ButtonGroup.Item>
      <ButtonGroup.Item>Rotate</ButtonGroup.Item>
    </ButtonGroup>
  );
}
export function Toggle() {
  const [on, setOn] = useState<Record<string, boolean>>({ wrap: true, time: false, ids: true });
  return (
    <ButtonGroup size="sm" aria-label="Log display">
      {[
        ["wrap", "Wrap lines"],
        ["time", "Timestamps"],
        ["ids", "Request IDs"],
      ].map(([id = "", label]) => (
        <ButtonGroup.Item key={id} pressed={!!on[id]} onClick={() => setOn((o) => ({ ...o, [id]: !o[id] }))}>
          {label}
        </ButtonGroup.Item>
      ))}
    </ButtonGroup>
  );
}
export function Icons() {
  return (
    <ButtonGroup size="sm" aria-label="Zoom">
      <ButtonGroup.Item icon={icons.minus} aria-label="Zoom out" />
      <ButtonGroup.Item>100%</ButtonGroup.Item>
      <ButtonGroup.Item icon={icons.add} aria-label="Zoom in" />
    </ButtonGroup>
  );
}
const KINDS = ["attached", "spaced", "icons"] as const;
const ITEM_STATES: [string, string, Record<string, unknown>][] = [
  ["Rest", "rest", {}],
  ["Hover", "hover", { "data-force": "hover" }],
  ["Focus", "focus", { "data-force": "focus" }],
  ["Pressed", "pressed", { "data-force": "pressed" }],
  ["Selected", "selected", { pressed: true }],
  ["Selected, hover", "selected-hover", { pressed: true, "data-force": "hover" }],
  ["Selected, focus", "selected-focus", { pressed: true, "data-force": "focus" }],
  ["Selected, pressed", "selected-pressed", { pressed: true, "data-force": "pressed" }],
  ["Disabled", "disabled", { disabled: true }],
  ["Selected, disabled", "selected-disabled", { pressed: true, disabled: true }],
];

function Group({ kind, props }: { kind: (typeof KINDS)[number]; props: Record<string, unknown> }) {
  if (kind === "icons")
    return (
      <ButtonGroup size="sm" aria-label="Layout">
        <ButtonGroup.Item icon={icons.list} aria-label="List" pressed={false} />
        <ButtonGroup.Item icon={icons.board} aria-label="Board" pressed={false} {...props} />
        <ButtonGroup.Item icon={icons.stack} aria-label="Stack" pressed={false} />
      </ButtonGroup>
    );
  return (
    <ButtonGroup size="sm" attached={kind === "attached"} aria-label="Period">
      <ButtonGroup.Item pressed={false}>1h</ButtonGroup.Item>
      <ButtonGroup.Item pressed={false} {...props}>
        24h
      </ButtonGroup.Item>
      <ButtonGroup.Item pressed={false}>7d</ButtonGroup.Item>
    </ButtonGroup>
  );
}

export function States() {
  return (
    <>
      <StateMatrix title="Middle item: states × kinds" columns={["Connected", "Spaced", "Icons"]}>
        {ITEM_STATES.map(([label, id, p]) => (
          <StateRow key={id} label={label}>
            {KINDS.map((k) => (
              <StateCell key={k} id={`${k}-${id}`}>
                <Group kind={k} props={p} />
              </StateCell>
            ))}
          </StateRow>
        ))}
      </StateMatrix>
      <StateMatrix title="Ends: focus and selection follow the outer shape" columns={["First focused", "Last selected", "Actions, no toggle"]}>
        <StateRow label="connected">
          <StateCell>
            <ButtonGroup size="sm" aria-label="Period">
              <ButtonGroup.Item pressed={false} data-force="focus">
                Day
              </ButtonGroup.Item>
              <ButtonGroup.Item pressed={false}>Week</ButtonGroup.Item>
            </ButtonGroup>
          </StateCell>
          <StateCell>
            <ButtonGroup size="sm" aria-label="Period">
              <ButtonGroup.Item pressed={false}>Day</ButtonGroup.Item>
              <ButtonGroup.Item pressed>Week</ButtonGroup.Item>
            </ButtonGroup>
          </StateCell>
          <StateCell>
            <ButtonGroup size="sm" aria-label="Export">
              <ButtonGroup.Item>CSV</ButtonGroup.Item>
              <ButtonGroup.Item>JSON</ButtonGroup.Item>
            </ButtonGroup>
          </StateCell>
        </StateRow>
      </StateMatrix>
      <StateSection title="Sizes">
        <div className="flex flex-col items-start gap-12">
          {(["xs", "sm", "md", "lg"] as const).map((s) => (
            <ButtonGroup key={s} size={s} aria-label="Period">
              <ButtonGroup.Item pressed={false}>Day</ButtonGroup.Item>
              <ButtonGroup.Item pressed>Week</ButtonGroup.Item>
              <ButtonGroup.Item icon={icons.calendar} aria-label="Custom range" />
            </ButtonGroup>
          ))}
        </div>
      </StateSection>
    </>
  );
}
