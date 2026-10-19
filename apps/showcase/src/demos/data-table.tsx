import type * as React from "react";
import { useMemo, useState } from "react";
import {
  DataTable,
  col,
  StatusCode,
  ModelLabel,
  Menu,
  Sheet,
  KeyValue,
  RequestTimeline,
  Button,
  IconButton,
  Pill,
  VisuallyHidden,
  icons,
  type ColumnDef,
} from "@logicplanes/kaeso-ui";
import { keys, makeLogs, members, models, NOW, type KeyRow, type LogRow, type ModelRow } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

const logs = makeLogs(200);
const columns: ColumnDef<LogRow, any>[] = [
  col.relative({ id: "time", header: "Time", size: 120, priority: 2 }, (r) => r.time, { now: NOW }),
  col.custom(
    { id: "status", header: "Status", size: 96, priority: 1 },
    (r) => r.status,
    (v) => <StatusCode code={v} />,
  ),
  col.custom(
    { id: "model", header: "Model", size: 240, priority: 1, grow: true },
    (r) => r.model,
    (v, r) => <ModelLabel provider={r.provider} model={v} />,
  ),
  col.text({ id: "key", header: "Key", size: 140, priority: 3 }, (r) => r.key),
  col.mono({ id: "endpoint", header: "Endpoint", size: 180, priority: 4 }, (r) => r.endpoint),
  col.tokens({ id: "in", header: "Input", size: 96, priority: 2 }, (r) => r.inputTokens),
  col.tokens({ id: "out", header: "Output", size: 96, priority: 2 }, (r) => r.outputTokens),
  col.duration({ id: "latency", header: "Latency", size: 104, priority: 2 }, (r) => r.latencyMs),
  col.custom(
    { id: "stream", header: "Stream", size: 104, priority: 2 },
    (r) => r.streamed,
    (v) =>
      v ? (
        <Pill size="sm" variant="outline">
          Stream
        </Pill>
      ) : null,
  ),
  col.money({ id: "cost", header: "Cost", size: 112, priority: 2 }, (r) => r.cost, { decimals: 4 }),
];

const STATE_LABEL: Record<KeyRow["state"], string> = { active: "Active", disabled: "Disabled", expired: "Expired", archived: "Archived" };
const keyColumns: ColumnDef<KeyRow, any>[] = [
  col.text({ id: "name", header: "Name", size: 180, priority: 1, grow: true }, (k) => k.name),
  col.text(
    { id: "state", header: "State", size: 120, priority: 1, tone: (k) => (k.state === "expired" ? "danger" : k.state === "disabled" ? "warning" : undefined) },
    (k) => STATE_LABEL[k.state],
  ),
  col.text({ id: "owner", header: "Owner", size: 160, priority: 2 }, (k) => k.owner),
  col.relative({ id: "used", header: "Last used", size: 120, priority: 2 }, (k) => k.lastUsed, { now: NOW }),
];

type Member = (typeof members)[number];
const memberColumns: ColumnDef<Member, any>[] = [
  col.text({ id: "name", header: "Name", size: 200, priority: 1, grow: true }, (m) => m.name),
  col.text({ id: "email", header: "Email", size: 240, priority: 2 }, (m) => m.email),
  col.text({ id: "preset", header: "Preset", size: 120, priority: 2 }, (m) => m.preset),
  col.custom(
    { id: "access", header: "Access", size: 160, priority: 1 },
    (m) => m.id,
    () => <Button size="sm">Review access</Button>,
  ),
];

function RowActions({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <Menu>
      <Menu.Trigger>
        <IconButton icon={icons.more} aria-label={label} size="xs" />
      </Menu.Trigger>
      <Menu.Content align="end">{children}</Menu.Content>
    </Menu>
  );
}
const actionsColumn = <T,>(label: (row: T) => string, items: (row: T) => React.ReactNode): ColumnDef<T, any> =>
  col.custom(
    { id: "actions", header: <VisuallyHidden>Actions</VisuallyHidden>, size: 64, priority: 1 },
    (r: T) => r,
    (r: T) => <RowActions label={label(r)}>{items(r)}</RowActions>,
    { align: "end" },
  );

const phoneColumns: ColumnDef<LogRow, any>[] = [
  col.relative({ id: "time", header: "Time", size: 104, priority: 1 }, (r) => r.time, { now: NOW }),
  col.custom(
    { id: "status", header: "Status", size: 80, priority: 1 },
    (r) => r.status,
    (v) => <StatusCode code={v} />,
  ),
  col.custom(
    { id: "model", header: "Model", size: 240, priority: 1, grow: true },
    (r) => r.model,
    (v, r) => <ModelLabel provider={r.provider} model={v} />,
  ),
  col.text({ id: "key", header: "Key", size: 140, priority: 3 }, (r) => r.key),
  col.duration({ id: "latency", header: "Latency", size: 104, priority: 2 }, (r) => r.latencyMs),
  col.money({ id: "cost", header: "Cost", size: 112, priority: 2 }, (r) => r.cost, { decimals: 4 }),
  actionsColumn<LogRow>(
    (r) => `Actions for ${r.id}`,
    (r) => (
      <>
        <Menu.Item icon={icons.copy}>Copy request id</Menu.Item>
        <Menu.Item icon={icons.filter}>Filter by {r.model}</Menu.Item>
      </>
    ),
  ),
];

const modelColumns: ColumnDef<ModelRow, any>[] = [
  col.custom(
    { id: "model", header: "Model", size: 320, priority: 1, grow: true },
    (m) => m.model,
    (v, m) => <ModelLabel provider={m.provider} model={v} name={m.name} />,
  ),
  col.money({ id: "input", header: "Input / 1M", size: 112, priority: 2 }, (m) => m.input, { decimals: 2 }),
  col.money({ id: "output", header: "Output / 1M", size: 112, priority: 2 }, (m) => m.output, { decimals: 2 }),
  actionsColumn<ModelRow>(
    (m) => `Actions for ${m.name}`,
    () => (
      <>
        <Menu.Item icon={icons.copy}>Copy model id</Menu.Item>
        <Menu.Item icon={icons.delete} variant="danger">
          Remove from project
        </Menu.Item>
      </>
    ),
  ),
];

export const meta = {
  title: "DataTable",
  description: "The console's table: a 36px header, 40px rows without lines, numbers right, sorting, resizing, selection, virtualization.",
  group: "Components",
  entry: "data-table",
  import: "DataTable, col",
  keyboard: [
    ["Tab", "Moves into the table, to the row last in focus, and out again"],
    ["Up Down", "Move between rows"],
    ["Home End", "First and last row"],
    ["Page Up Page Down", "A screen of rows up or down"],
    ["Enter", "Opens the row"],
    ["Space", "Selects the row, in a selectable table"],
    ["Left Right", "Resize a column, on its handle"],
    ["shift F10", "Opens the row menu"],
  ],
  a11y: [
    "role=table with column headers; sorted columns carry aria-sort.",
    "Columns declare a priority; low-priority columns hide first when space is short. A growing column then shrinks to its readable minimum (minWidth, 136px by default) and truncates. On a screen narrower still, the other priority 1 columns give way from the row's end backwards, keeping its last column (the actions) and the selection boxes; only a row with nothing left to give way scrolls sideways, its last column pinned at the end.",
    "Over 100 rows render virtually; the row count is in aria-rowcount and each row's place in aria-rowindex.",
  ],
};
export const examples = [
  { id: "logs", title: "Request logs", fn: "Logs", minHeight: 520 },
  {
    id: "narrow",
    title: "Any width",
    fn: "Narrow",
    description:
      "Narrow the window: the key, latency and cost drop first, the model keeps a readable width with its name cut short, and the row's actions stay at its end.",
  },
  {
    id: "tones",
    title: "Tones",
    fn: "Tones",
    description: "A column's tone colors the words that already carry a status: an expired or disabled key. Use it sparingly.",
  },
  {
    id: "states",
    title: "Loading, empty and error",
    fn: "LoadingStates",
    description: "A failed load says so where the rows would be and offers Try again; the error itself is a toast.",
  },
];
export function Logs() {
  const [open, setOpen] = useState<LogRow | null>(null);
  const [n, setN] = useState(60);
  const data = useMemo(() => logs.slice(0, n), [n]);
  return (
    <div className="h-480 w-full overflow-y-auto scrollbar-quiet">
      <DataTable
        aria-label="Request logs"
        data={data}
        columns={columns}
        getRowId={(r) => r.id}
        selectable
        activeRowId={open?.id}
        onRowClick={setOpen}
        rowMenu={(r) => (
          <>
            <Menu.Item icon={icons.openExternal} onClick={() => setOpen(r)}>
              Open request
            </Menu.Item>
            <Menu.Item icon={icons.copy}>Copy request id</Menu.Item>
            <Menu.Item icon={icons.filter}>Filter by {r.model}</Menu.Item>
          </>
        )}
        hasMore={n < logs.length}
        onLoadMore={() => setN((x) => x + 60)}
      />
      <Sheet
        open={!!open}
        onOpenChange={(o) => !o && setOpen(null)}
        title={open ? `${open.method} ${open.endpoint}` : ""}
        subtitle={open?.id}
        size="md"
        modal={false}
      >
        {open && (
          <div className="flex flex-col gap-24">
            <RequestTimeline ttfbMs={open.ttfbMs} totalMs={open.latencyMs} />
            <KeyValue
              items={[
                { label: "Status", value: <StatusCode code={open.status} showText /> },
                { label: "Model", value: open.model, mono: true },
                { label: "Key", value: open.key },
                { label: "Cost", value: `$${open.cost}` },
              ]}
            />
          </div>
        )}
      </Sheet>
    </div>
  );
}
export function Narrow() {
  return (
    <div className="flex w-full flex-col gap-32">
      <DataTable aria-label="Request logs" data={logs.slice(0, 6)} columns={phoneColumns} getRowId={(r) => r.id} />
      <DataTable aria-label="Models" data={models} columns={modelColumns} getRowId={(m) => m.id} />
    </div>
  );
}
function Phone({ width = 390, children }: { width?: number; children: React.ReactNode }) {
  return <div style={{ width }}>{children}</div>;
}
export function Tones() {
  return (
    <div className="w-full">
      <DataTable aria-label="Keys" data={keys} columns={keyColumns} getRowId={(k) => k.id} />
    </div>
  );
}
export function LoadingStates() {
  return (
    <div className="flex w-full flex-col gap-32">
      <DataTable aria-label="Loading" data={[]} columns={columns.slice(0, 5)} loading />
      <DataTable aria-label="Empty" data={[]} columns={columns.slice(0, 5)} />
      <DataTable
        aria-label="Error"
        data={[]}
        columns={columns.slice(0, 5)}
        error="The gateway answered 503. Check the gateway logs."
        onRetry={() => undefined}
      />
    </div>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Rows">
        <div className="w-1200">
          <DataTable
            aria-label="Logs"
            data={logs.slice(0, 8)}
            columns={columns}
            getRowId={(r) => r.id}
            selectable
            selection={{ [logs[1]!.id]: true }}
            activeRowId={logs[3]!.id}
            sorting={[{ id: "latency", desc: true }]}
          />
        </div>
      </StateSection>
      <StateSection title="Secondary controls on a row's fill (the active and selected rows)">
        <div className="w-960">
          <DataTable
            aria-label="Members"
            data={members.slice(0, 4)}
            columns={memberColumns}
            getRowId={(m) => m.id}
            selectable
            selection={{ [members[1]!.id]: true }}
            activeRowId={members[2]!.id}
          />
        </div>
      </StateSection>
      <StateSection title="Densities">
        <div className="flex w-960 flex-col gap-24">
          {(["compact", "default", "comfortable"] as const).map((d) => (
            <DataTable key={d} aria-label={d} density={d} data={logs.slice(0, 3)} columns={columns.slice(0, 6)} />
          ))}
        </div>
      </StateSection>
      <StateSection title="Tones on a column, on the panel and on hovered and selected rows">
        <div className="w-960">
          <DataTable
            aria-label="Keys"
            data={keys}
            columns={keyColumns}
            getRowId={(k) => k.id}
            selectable
            selection={{ [keys[4]!.id]: true }}
            activeRowId={keys[3]!.id}
          />
        </div>
      </StateSection>
      <StateSection title="At a phone's width (390px): the lower-priority columns drop, the model keeps a readable width and truncates, the actions stay">
        <div className="flex flex-wrap items-start gap-40">
          <Phone>
            <DataTable aria-label="Request logs at 390px" data={logs.slice(0, 6)} columns={phoneColumns} getRowId={(r) => r.id} activeRowId={logs[2]!.id} />
          </Phone>
          <Phone>
            <DataTable aria-label="Models at 390px" data={models} columns={modelColumns} getRowId={(m) => m.id} />
          </Phone>
        </div>
      </StateSection>
      <StateSection title="Narrower than the columns that always stay (320px): the status gives way so the model stays readable; the actions stay">
        <Phone width={320}>
          <DataTable aria-label="Request logs at 320px" data={logs.slice(0, 6)} columns={phoneColumns} getRowId={(r) => r.id} activeRowId={logs[2]!.id} />
        </Phone>
      </StateSection>
      <StateSection title="Loading, empty and error">
        <div className="w-960">
          <LoadingStates />
        </div>
      </StateSection>
    </>
  );
}
