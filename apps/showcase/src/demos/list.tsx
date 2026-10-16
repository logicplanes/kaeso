import type { ReactNode } from "react";
import { List, Avatar, Badge, Button, Empty, IconButton, InsetContent, SelectionAction, icons, type ListGroup, type ListItemData } from "@logicplanes/kaeso-ui";
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
    ["Up Down", "Move (also j k)"],
    ["Enter", "Opens the row"],
    ["x", "Selects the row"],
    ["shift Up", "Extends the selection"],
    ["mod A", "Selects all"],
    ["Escape", "Clears the selection"],
  ],
  a11y: [
    "An ARIA grid of one-cell rows, so rows can hold actions; selection is aria-selected and announced.",
    "While loading, the grid is aria-busy and shows rows shaped like items.",
    "Hover actions are also reachable by keyboard: they appear when the row has focus.",
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
export function States() {
  return (
    <>
      <StateSection title="Rows (first row forced)">
        <div className="flex w-960 flex-col gap-24">
          <List aria-label="Rest" items={groups[0]!.items.slice(0, 2)} />
          <List aria-label="Hover" items={groups[0]!.items.slice(0, 2)} data-force-row="hover" />
          <List aria-label="Focus" items={groups[0]!.items.slice(0, 2)} data-force-row="focus" />
          <List aria-label="Selected" items={groups[0]!.items.slice(0, 2)} selectable selected={new Set([groups[0]!.items[0]!.id])} />
        </div>
      </StateSection>
      <StateSection title="Loading, and selection without the floating bar">
        <div className="flex w-960 flex-col gap-24">
          <List aria-label="Loading" items={[]} loading />
          <List
            aria-label="Selected in a dialog"
            items={groups[0]!.items.slice(0, 2)}
            selectable
            selectionBar={false}
            selected={new Set([groups[0]!.items[0]!.id])}
          />
        </div>
      </StateSection>
      <StateSection title="Secondary controls on a row's fill (hover, selected), on the panel and in a card">
        <div className="flex flex-col gap-24">
          <div className="flex w-960 flex-col gap-24">
            <List aria-label="Invitations hover" items={invitations} data-force-row="hover" />
            <List aria-label="Invitations selected" items={invitations} selectable selectionBar={false} selected={new Set(["inv_1"])} data-force-row="hover" />
          </div>
          <Column>
            <List aria-label="Invitations in a card" items={invitations} data-force-row="hover" />
          </Column>
        </div>
      </StateSection>
      <StateSection title="In a settings column: stacked rows, and long titles at phone width">
        <div className="flex flex-col gap-24">
          <Column>
            <List aria-label="Members" items={memberRows.slice(0, 3)} data-force-row="hover" />
          </Column>
          <Column width="w-360">
            <List
              aria-label="Members, narrow"
              items={[
                {
                  id: "a",
                  title: "Ada Okafor, platform and billing administrator",
                  subtitle: "ada.okafor@northwind-laboratories.example",
                  meta: <Badge>Owner</Badge>,
                },
                { id: "b", title: "checkout-assistant-production-worker", subtitle: "Service account · created by Lin Park", meta: <Badge>Developer</Badge> },
              ]}
            />
          </Column>
        </div>
      </StateSection>
      <StateSection title="Empty: in a card, and on the panel">
        <div className="flex flex-col gap-24">
          <Column>
            <List
              aria-label="Invitations, none"
              items={[]}
              empty={<Empty size="md" title="No pending invitations" description="Accepted, revoked and expired invitations are under All." />}
            />
          </Column>
          <div className="w-760">
            <List aria-label="Keys, none" items={[]} empty={<Empty size="compact" title="No keys match" description="Clear the search to see every key." />} />
          </div>
        </div>
      </StateSection>
      <StateSection title="Groups and child rows">
        <div className="w-960">
          <List
            aria-label="Tree"
            groups={[
              {
                id: "g",
                title: "Aliases",
                count: 1,
                items: [
                  { id: "a", title: "checkout-default", displayId: "gpt-5-mini", children: [{ id: "b", title: "Fallback", displayId: "claude-haiku-4-5" }] },
                ],
              },
            ]}
          />
        </div>
      </StateSection>
    </>
  );
}
