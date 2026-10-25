import { ClientButton } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "ClientButton",
  description: "An agent client's mark on a raised tile, such as Claude, Cursor or GitHub Copilot, that opens how to connect it.",
  group: "Kaeso",
  entry: "kaeso",
  import: "ClientButton",
  a11y: ["A button named by `aria-label`, which is also its tooltip; the mark is decorative."],
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

export function States() {
  return (
    <StateSection title="Every client">
      <Clients />
    </StateSection>
  );
}
