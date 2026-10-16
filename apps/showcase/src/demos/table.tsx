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
    </>
  );
}
