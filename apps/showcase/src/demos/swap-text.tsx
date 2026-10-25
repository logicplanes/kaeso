import { useState } from "react";
import { SwapText, Switch } from "@logicplanes/kaeso-ui";
import { StateMatrix, StateRow, StateCell } from "~/lib/states";

const STATUSES = ["Streaming", "Done", "Failed"];

export const meta = {
  title: "SwapText",
  description: "Text that changes with state in a place sized for its widest version, so nothing around it moves.",
  group: "Components",
  entry: "swap-text",
  import: "SwapText",
  a11y: [
    "Only the current text is read; the reserved versions are hidden from screen readers.",
    "Buttons take reserveLabels, which wraps their label in SwapText; reach for SwapText itself for text outside a button.",
  ],
};
export const examples = [
  { id: "basic", title: "Basic", fn: "Basic" },
  {
    id: "start",
    title: "Aligned at the start",
    fn: "AlignStart",
    description: "In a column, start-aligned labels keep the text after them on one line.",
  },
];
export function Basic() {
  const [active, setActive] = useState(true);
  return (
    <div className="flex items-center gap-12 text-body">
      <Switch size="sm" aria-label="Key active" checked={active} onCheckedChange={setActive} />
      <span className="font-medium text-fg">
        <SwapText reserve={["Active", "Paused"]} align="start">
          {active ? "Active" : "Paused"}
        </SwapText>
      </span>
      <span className="text-fg-secondary">production</span>
    </div>
  );
}
export function AlignStart() {
  return (
    <div className="flex flex-col gap-8 text-body">
      {[
        { status: "Streaming", model: "gpt-5-mini" },
        { status: "Done", model: "claude-sonnet-4-5" },
        { status: "Failed", model: "gpt-4.1-nano" },
      ].map((row) => (
        <div key={row.model} className="flex items-center gap-12">
          <SwapText reserve={STATUSES} align="start" className="font-medium text-fg">
            {row.status}
          </SwapText>
          <span className="mono text-fg-secondary">{row.model}</span>
        </div>
      ))}
    </div>
  );
}
export function States() {
  return (
    <StateMatrix title="Alignment × text" columns={STATUSES}>
      {(["center", "start"] as const).map((align) => (
        <StateRow key={align} label={align}>
          {STATUSES.map((status) => (
            <StateCell key={status}>
              <span className="inline-flex h-32 items-center rounded-pill bg-fill-1 px-12 text-body font-medium text-fg">
                <SwapText reserve={STATUSES} align={align}>
                  {status}
                </SwapText>
              </span>
            </StateCell>
          ))}
        </StateRow>
      ))}
    </StateMatrix>
  );
}
