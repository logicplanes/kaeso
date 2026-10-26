import { useEffect, useRef } from "react";
import { Tabs, PillTabs, InsetContent, icons } from "@logicplanes/kaeso-ui";
import { StateCell, StateMatrix, StateRow, StateSection } from "~/lib/states";

export const meta = {
  title: "Tabs",
  description: "Underline tabs for detail pages and pill tabs for page headers.",
  group: "Components",
  entry: "tabs",
  import: "Tabs, PillTabs",
  keyboard: [
    ["Left Right", "Moves focus between tabs"],
    ["Home End", "Moves focus to the first or last tab"],
    ["Enter Space", "Opens the focused tab"],
  ],
  a11y: [
    "role=tablist with tabs and panels wired by Base UI; arrows move focus and Enter or Space opens the tab, so a slow panel never loads on the way past.",
    "Focus rings sit inside the pill, so a scrolling tab list never cuts them; in forced colours the selected tab keeps a Highlight outline and the indicator a solid line.",
  ],
};
export const examples = [
  { id: "underline", title: "Underline", fn: "Underline" },
  { id: "pill", title: "Pill tabs", fn: "Pills" },
];
export function Underline() {
  return (
    <Tabs defaultValue="overview" className="w-480">
      <Tabs.List aria-label="Request">
        <Tabs.Tab value="overview">Overview</Tabs.Tab>
        <Tabs.Tab value="conversation">Conversation</Tabs.Tab>
        <Tabs.Tab value="request">Request</Tabs.Tab>
        <Tabs.Tab value="response">Response</Tabs.Tab>
      </Tabs.List>
      <Tabs.Panel value="overview" className="pt-16 text-body text-fg-secondary">
        The gateway answered in 1.2 s.
      </Tabs.Panel>
    </Tabs>
  );
}
export function Pills() {
  return (
    <PillTabs defaultValue="all">
      <PillTabs.List aria-label="Keys">
        <PillTabs.Tab value="all" count={6}>
          All
        </PillTabs.Tab>
        <PillTabs.Tab value="active" count={3}>
          Active
        </PillTabs.Tab>
        <PillTabs.Tab value="disabled">Disabled</PillTabs.Tab>
        <PillTabs.Tab value="archived" icon={icons.stack}>
          Archived
        </PillTabs.Tab>
      </PillTabs.List>
    </PillTabs>
  );
}
type Force = "hover" | "pressed" | "focus" | undefined;

function OnePill({
  label,
  selected,
  force,
  disabled,
  count,
}: {
  label: string;
  selected?: boolean;
  force?: Force;
  disabled?: boolean | undefined;
  count?: number;
}) {
  return (
    <PillTabs value={selected ? "t" : "other"}>
      <PillTabs.List aria-label={label}>
        <PillTabs.Tab value="t" {...(force ? { "data-force": force } : {})} {...(disabled ? { disabled } : {})} {...(count !== undefined ? { count } : {})}>
          {label}
        </PillTabs.Tab>
      </PillTabs.List>
    </PillTabs>
  );
}

function OneUnderline({ label, selected, force, disabled }: { label: string; selected?: boolean; force?: Force; disabled?: boolean | undefined }) {
  return (
    <Tabs value={selected ? "t" : "other"}>
      <Tabs.List aria-label={label}>
        <Tabs.Tab value="t" {...(force ? { "data-force": force } : {})} {...(disabled ? { disabled } : {})}>
          {label}
        </Tabs.Tab>
      </Tabs.List>
    </Tabs>
  );
}

const COLUMNS = ["Rest", "Hover", "Pressed", "Focus", "Disabled"];
const FORCES: { id: string; force?: Force; disabled?: boolean }[] = [
  { id: "rest" },
  { id: "hover", force: "hover" },
  { id: "pressed", force: "pressed" },
  { id: "focus", force: "focus" },
  { id: "disabled", disabled: true },
];

export function States() {
  return (
    <>
      <StateMatrix title="Pill tabs" columns={COLUMNS}>
        <StateRow label="Not selected">
          {FORCES.map((f) => (
            <StateCell key={f.id} id={`pill-${f.id}`}>
              <OnePill label="Active" force={f.force} disabled={f.disabled} />
            </StateCell>
          ))}
        </StateRow>
        <StateRow label="Selected">
          {FORCES.map((f) => (
            <StateCell key={f.id} id={`pill-sel-${f.id}`}>
              <OnePill label="Active" selected force={f.force} disabled={f.disabled} />
            </StateCell>
          ))}
        </StateRow>
        <StateRow label="With count">
          <StateCell id="pill-count-rest">
            <OnePill label="All" count={6} />
          </StateCell>
          <StateCell id="pill-count-selected">
            <OnePill label="All" selected count={6} />
          </StateCell>
        </StateRow>
      </StateMatrix>
      <StateMatrix title="Underline tabs" columns={COLUMNS}>
        <StateRow label="Not selected">
          {FORCES.map((f) => (
            <StateCell key={f.id} id={`line-${f.id}`}>
              <OneUnderline label="Request" force={f.force} disabled={f.disabled} />
            </StateCell>
          ))}
        </StateRow>
        <StateRow label="Selected">
          {FORCES.map((f) => (
            <StateCell key={f.id} id={`line-sel-${f.id}`}>
              <OneUnderline label="Request" selected force={f.force} disabled={f.disabled} />
            </StateCell>
          ))}
        </StateRow>
      </StateMatrix>
      <StateSection title="Pill tabs in a settings column: the first pill on the title's edge">
        <div className="w-full max-w-480 bg-panel">
          <h2 className="mb-16 text-title-lg font-semibold text-fg">Keys</h2>
          <InsetContent>
            <Pills />
          </InsetContent>
        </div>
      </StateSection>
      <StateSection title="Underline tabs with icons, counts and a long label">
        <Tabs defaultValue="overview" className="w-full max-w-560">
          <Tabs.List aria-label="Request">
            <Tabs.Tab value="overview" icon={icons.stack}>
              Overview
            </Tabs.Tab>
            <Tabs.Tab value="logs" count={128}>
              Logs
            </Tabs.Tab>
            <Tabs.Tab value="latency">Antwortzeiten und Durchsatz</Tabs.Tab>
          </Tabs.List>
          <Tabs.Panel value="overview" data-force="focus" className="mt-8 rounded-item p-12 text-body text-fg-secondary">
            Panel focus: the gateway answered in 1.2 s.
          </Tabs.Panel>
        </Tabs>
      </StateSection>
      <StateSection title="Overflowing at 280px: scrolled to the middle, both edges fade">
        <Scrolled />
      </StateSection>
    </>
  );
}

function Scrolled() {
  const list = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (list.current) list.current.scrollLeft = 96;
  }, []);
  return (
    <div className="w-280">
      <Tabs defaultValue="response">
        <Tabs.List ref={list} aria-label="Request">
          <Tabs.Tab value="overview">Overview</Tabs.Tab>
          <Tabs.Tab value="conversation">Conversation</Tabs.Tab>
          <Tabs.Tab value="request">Request</Tabs.Tab>
          <Tabs.Tab value="response">Response</Tabs.Tab>
          <Tabs.Tab value="archive">Archive</Tabs.Tab>
        </Tabs.List>
      </Tabs>
    </div>
  );
}
