import { Icon, Timeline, icons } from "@logicplanes/kaeso-ui";
import { auditEvents } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

export const meta = { title: "Timeline", description: "Events down a 1px line: the audit log.", group: "Components", entry: "timeline", import: "Timeline" };
export const examples = [{ id: "audit", title: "Audit log", fn: "Audit" }];
export function Audit() {
  return <Timeline events={auditEvents} className="w-560" />;
}
export function States() {
  return (
    <>
      <StateSection title="Tones">
        <Audit />
      </StateSection>
      <StateSection title="Descriptions, a custom marker, a long title at 320px">
        <Timeline
          className="w-320"
          events={[
            {
              id: "a",
              title: "Ada Okafor created the key checkout-web for the checkout assistant production deployment",
              description: "Inference only, expires 1 Jan 2027",
              time: "14:12",
              datetime: "2026-10-12T14:12:00Z",
              tone: "success",
            },
            {
              id: "b",
              title: "Budget alert at 80%",
              time: "13:58",
              datetime: "2026-10-12T13:58:00Z",
              tone: "warning",
              marker: <Icon icon={icons.budget} size="sm" className="text-warning-text" />,
            },
            { id: "c", title: "Lin Park revoked the key staging", time: "09:40", datetime: "2026-10-12T09:40:00Z", tone: "danger" },
          ]}
        />
      </StateSection>
    </>
  );
}
