import { Timeline } from "@logicplanes/kaeso-ui";
import { auditEvents } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

export const meta = { title: "Timeline", description: "Events down a 1px line: the audit log.", group: "Components", entry: "timeline", import: "Timeline" };
export const examples = [{ id: "audit", title: "Audit log", fn: "Audit" }];
export function Audit() {
  return <Timeline events={auditEvents} className="w-560" />;
}
export function States() {
  return (
    <StateSection title="Tones">
      <Audit />
    </StateSection>
  );
}
