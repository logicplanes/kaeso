import { Button, ClientButton, IconButton, icons } from "@logicplanes/kaeso-ui";
import { Matrix, STATE_COLUMNS, StateSection } from "~/lib/states";

export const meta = {
  title: "ClientButton",
  description: "An agent client's mark in a raised circle, such as Claude, Cursor or GitHub Copilot, that opens how to connect it.",
  group: "Kaeso",
  entry: "kaeso",
  import: "ClientButton",
  a11y: ["A button named by `aria-label`, which is also its tooltip; the mark is decorative.", "A 32px circle with a 44px hit area on touch screens."],
};
export const examples = [{ id: "clients", title: "Clients", fn: "Clients" }];

export function Clients() {
  return (
    <div className="flex gap-8">
      <ClientButton client="claude" aria-label="Add to Claude Code" />
      <ClientButton client="cursor" aria-label="Add to Cursor" />
      <ClientButton client="github-copilot" aria-label="Add to GitHub Copilot" />
    </div>
  );
}

const CLIENTS = [
  ["claude", "Add to Claude Code"],
  ["cursor", "Add to Cursor"],
  ["github-copilot", "Add to GitHub Copilot"],
] as const;

export function States() {
  return (
    <>
      <Matrix
        title="Clients × states"
        rows={CLIENTS.map(([client, label]) => ({ label: client, client, name: label }))}
        columns={STATE_COLUMNS.filter((c) => c.label !== "Disabled")}
        render={(r, p) => <ClientButton client={r.client} aria-label={r.name} {...p} />}
      />
      <StateSection title="Beside other header buttons">
        <div className="flex items-center gap-8">
          <IconButton variant="secondary" icon={icons.help} aria-label="Help" />
          <ClientButton client="claude" aria-label="Add to Claude Code" />
          <Button size="sm">Connect</Button>
        </div>
      </StateSection>
    </>
  );
}
