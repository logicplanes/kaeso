import { Button, IconButton, Tooltip, TooltipSurface, icons } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Tooltip",
  description: "A short label on hover and focus: a pill, with a shortcut when there is one.",
  group: "Components",
  entry: "tooltip",
  import: "Tooltip",
  keyboard: [
    ["Tab", "Shows the tooltip of the focused control"],
    ["Escape", "Hides it"],
  ],
  a11y: [
    "Opens after 500ms, instantly when moving between tooltipped siblings.",
    "Never the only place for essential information.",
    "Disabled controls show a tooltip only with a disabledReason.",
    "role=tooltip. When it says more than the control's name, it is also the control's accessible description.",
    "A single-line tooltip too long for the screen wraps into the box of several lines.",
  ],
};
export const examples = [
  { id: "basic", title: "Basic", fn: "Basic" },
  { id: "shortcut", title: "With a shortcut", fn: "Shortcut", description: "The label and its keys together: “Go to request logs  G then L”." },
  { id: "multiline", title: "Several lines", fn: "Multiline" },
];
export function Basic() {
  return (
    <Tooltip content="Filter">
      <IconButton icon={icons.filter} aria-label="Filter" variant="secondary" tooltip={false} />
    </Tooltip>
  );
}
export function Shortcut() {
  return (
    <Tooltip content="Go to request logs" shortcut={[["g"], ["l"]]} defaultOpen>
      <Button variant="ghost">Request logs</Button>
    </Tooltip>
  );
}
export function Multiline() {
  return (
    <Tooltip content="Requests over a hard limit are refused with 429. Soft limits only send an alert." shape="multiline">
      <Button>Hard limit</Button>
    </Tooltip>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Surfaces">
        <div className="flex flex-col items-start gap-16">
          <TooltipSurface>Filter</TooltipSurface>
          <TooltipSurface shortcut={[["g"], ["l"]]}>Go to request logs</TooltipSurface>
          <TooltipSurface shortcut={["mod", "k"]}>Search</TooltipSurface>
          <TooltipSurface shape="multiline">Requests over a hard limit are refused with 429. Soft limits only send an alert.</TooltipSurface>
        </div>
      </StateSection>
      <StateSection title="Open on its control">
        <div className="flex h-160 items-end gap-160 pl-8">
          <Tooltip content="Filter" open>
            <IconButton icon={icons.filter} aria-label="Filter" variant="secondary" tooltip={false} />
          </Tooltip>
          <Tooltip content="Requests over a hard limit are refused with 429. Soft limits only send an alert." shape="multiline" open>
            <Button>Hard limit</Button>
          </Tooltip>
        </div>
      </StateSection>
    </>
  );
}
