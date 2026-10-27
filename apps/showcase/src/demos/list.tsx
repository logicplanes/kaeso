import type { ReactNode } from "react";
import {
  List,
  Avatar,
  Badge,
  Button,
  Empty,
  IconButton,
  InsetContent,
  PortalContainer,
  SelectionAction,
  icons,
  type ListGroup,
  type ListItemData,
} from "@logicplanes/kaeso-ui";
import { formatRelative } from "@logicplanes/kaeso-ui/format";
import { useLocale } from "@logicplanes/kaeso-ui/i18n";
import { keys, members, NOW } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

function LastUsed({ at }: { at: string | Date }) {
  useLocale();
  return <>{formatRelative(at, NOW)}</>;
}

const groups: ListGroup[] = (["active", "disabled", "expired"] as const).map((state) => ({
  id: state,
  title: state[0]!.toUpperCase() + state.slice(1),
  onAdd: state === "active" ? () => undefined : undefined,
  addLabel: "Create key",
  items: keys
    .filter((k) => k.state === state)
    .map((k) => ({
      id: k.id,
      title: k.name,
      subtitle: <span className="mono text-small">{k.redacted}</span>,
      meta: <span className="text-small text-fg-secondary">{k.access === "platform" ? "Platform" : "Inference"}</span>,
      date: <LastUsed at={k.lastUsed} />,
      avatar: <Avatar name={k.owner} size={20} decorative />,
      actions: (
        <>
          <IconButton icon={icons.copy} aria-label="Copy key id" size="xs" />
          <IconButton icon={icons.more} aria-label="More" size="xs" />
        </>
      ),
    })),
}));

export const meta = {
  title: "List",
  description: "44px rows on the panel, no lines, metadata on the right, group headers, keyboard first.",
  group: "Components",
  entry: "list",
  import: "List, ListRow, GroupHeader",
  keyboard: [
    ["Tab", "Enters the list on the last focused row, and the next Tab leaves it"],
    ["Up Down", "Moves between rows and group headers (also j k)"],
    ["Home End", "Moves to the first or the last row"],
    ["Space", "Selects or unselects the row (also x)"],
    ["Enter", "Opens the row; on a group header, collapses or expands the group"],
    ["Right Left", "Moves into the row's actions and back"],
    ["Escape", "Returns from an action to its row; on a row, clears the selection"],
    ["shift Up Down", "Extends the selection"],
    ["mod A", "Selects all"],
    ["mod Enter", "Left to the surrounding form, such as a dialog's submit"],
  ],
  a11y: [
    "An ARIA grid of one-cell rows with one tab stop: focus roves between rows and group headers, and checkboxes and row actions stay out of the tab order.",
    "Selection is aria-selected on the cell and only present when the list is selectable; disabled rows can be focused but not opened or selected.",
    "Row actions are reached with the right arrow and appear while their row has focus.",
    "While loading, the grid is aria-busy and shows rows shaped like items.",
    "Lists over 100 rows are virtualized.",
  ],
};
export const examples = [
  { id: "keys", title: "Keys by state", fn: "Keys" },
  { id: "settings", title: "In a settings column", fn: "InSettings" },
];

function Column({ width = "w-760", children }: { width?: string; children: ReactNode }) {
  return (
    <div className={`${width} bg-panel p-24`}>
      <InsetContent>{children}</InsetContent>
    </div>
  );
}
const invitations: ListItemData[] = [
  {
    id: "inv_1",
    title: "priya@northwind.example",
    subtitle: "Developer · invited by Ada Okafor",
    meta: <Badge>Pending</Badge>,
    actions: <Button size="sm">Resend</Button>,
  },
  {
    id: "inv_2",
    title: "sam@northwind.example",
    subtitle: "Viewer · invited by Lin Park",
    meta: <Badge>Pending</Badge>,
    actions: <Button size="sm">Resend</Button>,
  },
];
const memberRows: ListItemData[] = members.map((m) => ({
  id: m.id,
  title: m.name,
  subtitle: m.email,
  leading: <Avatar name={m.name} size={20} decorative />,
  meta: <Badge>{m.preset}</Badge>,
}));
export function InSettings() {
  return (
    <Column>
      <List aria-label="Members" items={memberRows} />
    </Column>
  );
}
export function Keys() {
  return (
    <div className="w-full">
      <List
        aria-label="Keys"
        groups={groups}
        selectable
        selectionActions={
          <>
            <SelectionAction icon={icons.copy}>Copy ids</SelectionAction>
            <SelectionAction icon={icons.hide}>Disable</SelectionAction>
            <SelectionAction icon={icons.delete} danger>
              Delete
            </SelectionAction>
          </>
        }
      />
    </div>
  );
}
function Cell({ id, label, children }: { id: string; label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-4">
      <span className="text-caption font-medium text-fg-tertiary">{label}</span>
      <div data-cell={id}>{children}</div>
    </div>
  );
}

const one = groups[0]!.items.slice(0, 1);
const firstId = one[0]!.id;
const ROW_STATES: { id: string; label: string; force?: string; selected?: boolean; disabled?: boolean }[] = [
  { id: "rest", label: "Rest" },
  { id: "hover", label: "Hover", force: "hover" },
  { id: "pressed", label: "Pressed", force: "pressed" },
  { id: "focus", label: "Keyboard focus", force: "focus" },
  { id: "disabled", label: "Disabled", disabled: true },
];
const SELECTED_STATES = [
  { id: "rest", label: "Selected" },
  { id: "hover", label: "Selected, hover", force: "hover" },
  { id: "pressed", label: "Selected, pressed", force: "pressed" },
];

function RowStates({ prefix }: { prefix: string }) {
  return (
    <div className="flex w-560 flex-col gap-12">
      {ROW_STATES.map((s) => (
        <Cell key={s.id} id={`${prefix}-${s.id}`} label={s.label}>
          <List
            aria-label={s.label}
            items={s.disabled ? one.map((i) => ({ ...i, disabled: true })) : one}
            selectable
            selectionBar={false}
            {...(s.force ? { "data-force-row": s.force } : {})}
          />
        </Cell>
      ))}
      {SELECTED_STATES.map((s) => (
        <Cell key={s.id} id={`${prefix}sel-${s.id}`} label={s.label}>
          <List
            aria-label={s.label}
            items={one}
            selectable
            selectionBar={false}
            selected={new Set([firstId])}
            {...(s.force ? { "data-force-row": s.force } : {})}
          />
        </Cell>
      ))}
    </div>
  );
}

const many: ListItemData[] = Array.from({ length: 140 }, (_, i) => ({
  id: `req_${i}`,
  title: `checkout-web request ${i + 1}`,
  displayId: `req_01JB${String(1000 + i)}`,
  date: `${(i % 59) + 1} min ago`,
}));

export function States() {
  return (
    <>
      <StateSection title="Rows on the panel">
        <RowStates prefix="panel" />
      </StateSection>
      <StateSection title="Rows in a card">
        <div>
          <InsetContent>
            <RowStates prefix="card" />
          </InsetContent>
        </div>
      </StateSection>
      <StateSection title="Secondary controls on a row's fill">
        <div className="flex w-560 flex-col gap-12">
          <List aria-label="Invitations hover" items={invitations} data-force-row="hover" />
          <List aria-label="Invitations selected" items={invitations} selectable selectionBar={false} selected={new Set(["inv_1"])} data-force-row="hover" />
          <InsetContent>
            <List aria-label="Invitations in a card" items={invitations} data-force-row="hover" />
          </InsetContent>
        </div>
      </StateSection>
      <StateSection title="Selecting, with the floating bar">
        <div className="relative h-160 w-560 contain-layout">
          <PortalContainer>
            <List
              aria-label="Selecting"
              items={groups[0]!.items.slice(0, 2)}
              selectable
              selected={new Set([firstId])}
              selectionActions={
                <>
                  <SelectionAction icon={icons.copy}>Copy ids</SelectionAction>
                  <SelectionAction icon={icons.delete} danger>
                    Delete
                  </SelectionAction>
                </>
              }
            />
          </PortalContainer>
        </div>
      </StateSection>
      <StateSection title="Groups: collapsed, with an add button, in a selectable list, loading">
        <div className="flex w-560 flex-col gap-24">
          <List
            aria-label="Keys by state"
            selectable
            selectionBar={false}
            groups={[
              { ...groups[0]!, items: groups[0]!.items.slice(0, 2) },
              { ...groups[1]!, defaultCollapsed: true },
            ]}
          />
          <List aria-label="Loading keys" groups={groups.map((g) => ({ ...g, items: [] }))} loading />
        </div>
      </StateSection>
      <StateSection title="Compact, ids, a child row, a disabled row, a wrapping description">
        <div className="flex w-560 flex-col gap-24">
          <List
            aria-label="Aliases"
            density="compact"
            idWidth={120}
            items={[
              { id: "a", title: "checkout-default", displayId: "gpt-5-mini", children: [{ id: "b", title: "Fallback", displayId: "claude-haiku-4-5" }] },
              { id: "c", title: "support-triage", displayId: "claude-sonnet-4-5", disabled: true, date: "Paused" },
            ]}
          />
          <InsetContent>
            <List
              aria-label="Models"
              wrapSubtitle
              items={[
                {
                  id: "m",
                  title: "Claude Sonnet 4.5",
                  subtitle:
                    "Anthropic's balanced model for agents and coding. Supports tools, vision, PDFs and extended reasoning with up to 64,000 output tokens.",
                },
              ]}
            />
          </InsetContent>
        </div>
      </StateSection>
      <StateSection title="At phone width: lines, stacked rows, long titles">
        <div className="flex flex-col gap-24">
          <div className="w-360">
            <List aria-label="Keys, narrow" items={groups[0]!.items.slice(0, 3)} />
          </div>
          <Column width="w-360">
            <List
              aria-label="Members, narrow"
              items={[
                {
                  id: "a",
                  title: "Ada Okafor, platform and billing administrator",
                  subtitle: "ada.okafor@northwind-laboratories.example",
                  leading: <Avatar name="Ada Okafor" size={20} decorative />,
                  meta: <Badge>Owner</Badge>,
                },
                { id: "b", title: "checkout-assistant-production-worker", subtitle: "Service account · created by Lin Park", meta: <Badge>Developer</Badge> },
              ]}
            />
          </Column>
        </div>
      </StateSection>
      <StateSection title="Empty: in a card, and on the panel">
        <div className="flex w-560 flex-col gap-24">
          <InsetContent>
            <List
              aria-label="Invitations, none"
              items={[]}
              empty={<Empty size="md" title="No pending invitations" description="Accepted, revoked and expired invitations are under All." />}
            />
          </InsetContent>
          <List aria-label="Keys, none" items={[]} empty={<Empty size="compact" title="No keys match" description="Clear the search to see every key." />} />
        </div>
      </StateSection>
      <StateSection title="Over 100 rows: virtualized in its scroller">
        <div className="h-240 w-560 overflow-y-auto">
          <List aria-label="Requests" items={many} />
        </div>
      </StateSection>
    </>
  );
}
