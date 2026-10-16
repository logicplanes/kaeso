import { StatusDot } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "StatusDot",
  description: "An 8px semantic dot with its word. Status is never color alone.",
  group: "Components",
  entry: "status-dot",
  import: "StatusDot",
};
export const examples = [{ id: "tones", title: "Tones", fn: "Tones" }];
export function Tones() {
  return (
    <div className="flex flex-col gap-8">
      <StatusDot tone="success">Active</StatusDot>
      <StatusDot tone="warning">Expiring</StatusDot>
      <StatusDot tone="danger">Down</StatusDot>
      <StatusDot tone="info">Syncing</StatusDot>
      <StatusDot tone="neutral">Archived</StatusDot>
      <StatusDot tone="success">Live</StatusDot>
    </div>
  );
}
export function States() {
  return (
    <StateSection title="Plain and colored labels">
      <div className="flex gap-48">
        <Tones />
        <div className="flex flex-col gap-8">
          {(["success", "warning", "danger", "info", "neutral"] as const).map((t) => (
            <StatusDot key={t} tone={t} size="sm">
              {t}
            </StatusDot>
          ))}
        </div>
      </div>
    </StateSection>
  );
}
