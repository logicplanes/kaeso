import { Link, Table } from "@logicplanes/kaeso-ui";
import { models } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Table",
  description: "A plain hairline table for small static data.",
  group: "Components",
  entry: "table",
  import: "Table",
};
export const examples = [
  { id: "prices", title: "Prices", fn: "Prices" },
  { id: "tones", title: "Tones", fn: "Tones", description: "A cell's tone colors words that already carry a status. Use it sparingly." },
  {
    id: "wrapping",
    title: "Wrapping",
    fn: "Wrapping",
    description: "Long text and links wrap like prose; the row grows and keeps its padding, and one-line rows stay 44px.",
  },
  {
    id: "compact",
    title: "Compact",
    fn: "Compact",
    description: "A reference table of five or more columns puts 8px between them, so it fits the column without scrolling.",
  },
];
const limits = [
  {
    key: "checkout-web",
    limit: "600 requests a minute",
    note: "Requests over the limit get a 429 with the seconds to wait.",
    href: "https://docs.kaeso.app/console/limits-and-budgets",
    link: "How limits and budgets work",
  },
  {
    key: "ci-runner",
    limit: "$50.00 a month",
    note: "The key stops at its budget until the month resets or an owner raises it.",
    href: "https://docs.kaeso.app/console/api-keys",
    link: "Raise a key's budget",
  },
  {
    key: "checkout-worker",
    limit: "None",
    note: "Uses the project's limits.",
    href: "https://docs.kaeso.app/console/limits-and-budgets",
    link: "Project limits",
  },
] as const;
export function Wrapping() {
  return (
    <Table className="w-560">
      <Table.Head>
        <Table.Row>
          <Table.HeaderCell>Key</Table.HeaderCell>
          <Table.HeaderCell>Limit</Table.HeaderCell>
          <Table.HeaderCell>Note</Table.HeaderCell>
        </Table.Row>
      </Table.Head>
      <Table.Body>
        {limits.map((l) => (
          <Table.Row key={l.key}>
            <Table.Cell className="mono text-small whitespace-nowrap">{l.key}</Table.Cell>
            <Table.Cell>{l.limit}</Table.Cell>
            <Table.Cell>
              {l.note} <Link href={l.href}>{l.link}</Link>
            </Table.Cell>
          </Table.Row>
        ))}
      </Table.Body>
    </Table>
  );
}
const providers = [
  { name: "OpenAI", state: "Reachable", tone: undefined, latency: "412 ms" },
  { name: "Anthropic", state: "Slower than usual", tone: "warning", latency: "2.1 s" },
  { name: "Northwind vLLM", state: "Unreachable", tone: "danger", latency: "—" },
] as const;
const settings = [
  {
    variable: "KAESO_LISTEN",
    key: "listen",
    type: "text",
    default: "0.0.0.0:8080",
    does: "The address of the model API, the Platform API and the health checks.",
  },
  {
    variable: "KAESO_METRICS_LISTEN",
    key: "metrics_listen",
    type: "text",
    default: "127.0.0.1:9100",
    does: "The address of the Prometheus metrics. Never forward it publicly.",
  },
  { variable: "KAESO_TRUSTED_PROXIES", key: "trusted_proxies", type: "list", default: "none", does: "Networks whose X-Forwarded-For the gateway believes." },
] as const;
export function Compact() {
  return (
    <Table density="compact" className="w-640">
      <Table.Head>
        <Table.Row>
          <Table.HeaderCell>Variable</Table.HeaderCell>
          <Table.HeaderCell>TOML key</Table.HeaderCell>
          <Table.HeaderCell>Type</Table.HeaderCell>
          <Table.HeaderCell>Default</Table.HeaderCell>
          <Table.HeaderCell>What it does</Table.HeaderCell>
        </Table.Row>
      </Table.Head>
      <Table.Body>
        {settings.map((s) => (
          <Table.Row key={s.variable}>
            <Table.Cell className="mono text-small">{s.variable}</Table.Cell>
            <Table.Cell className="mono text-small">{s.key}</Table.Cell>
            <Table.Cell>{s.type}</Table.Cell>
            <Table.Cell className="mono text-small whitespace-nowrap">{s.default}</Table.Cell>
            <Table.Cell>{s.does}</Table.Cell>
          </Table.Row>
        ))}
      </Table.Body>
    </Table>
  );
}
export function Tones() {
  return (
    <Table className="w-560">
      <Table.Head>
        <Table.Row>
          <Table.HeaderCell>Provider</Table.HeaderCell>
          <Table.HeaderCell>State</Table.HeaderCell>
          <Table.HeaderCell align="end">Median latency</Table.HeaderCell>
        </Table.Row>
      </Table.Head>
      <Table.Body>
        {providers.map((p) => (
          <Table.Row key={p.name}>
            <Table.Cell>{p.name}</Table.Cell>
            <Table.Cell tone={p.tone}>{p.state}</Table.Cell>
            <Table.Cell align="end">{p.latency}</Table.Cell>
          </Table.Row>
        ))}
      </Table.Body>
    </Table>
  );
}
export function Prices() {
  return (
    <Table className="w-640">
      <Table.Head>
        <Table.Row>
          <Table.HeaderCell>Model</Table.HeaderCell>
          <Table.HeaderCell align="end">Input / M</Table.HeaderCell>
          <Table.HeaderCell align="end">Output / M</Table.HeaderCell>
          <Table.HeaderCell align="end">Cache read / M</Table.HeaderCell>
        </Table.Row>
      </Table.Head>
      <Table.Body>
        {models.map((m) => (
          <Table.Row key={m.id}>
            <Table.Cell className="mono text-small">{m.model}</Table.Cell>
            <Table.Cell align="end">${m.input}</Table.Cell>
            <Table.Cell align="end">${m.output}</Table.Cell>
            <Table.Cell align="end">${m.cacheRead}</Table.Cell>
          </Table.Row>
        ))}
      </Table.Body>
    </Table>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Table">
        <Prices />
      </StateSection>
      <StateSection title="Tones">
        <Tones />
      </StateSection>
      <StateSection title="Wrapping">
        <Wrapping />
      </StateSection>
      <StateSection title="Compact">
        <Compact />
      </StateSection>
    </>
  );
}
