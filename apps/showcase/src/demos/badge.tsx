import { Badge } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Badge",
  description: "A small pill label: 22px, 12/500. Words carry the meaning.",
  group: "Components",
  entry: "badge",
  import: "Badge",
};
export const examples = [{ id: "tones", title: "Tones", fn: "Tones" }];
export function Tones() {
  return (
    <div className="flex flex-wrap items-center gap-8">
      <Badge>Inference</Badge>
      <Badge tone="success">Active</Badge>
      <Badge tone="warning">Expiring</Badge>
      <Badge tone="danger">Failed</Badge>
      <Badge tone="info">Syncing</Badge>
      <Badge tone="outline">Platform</Badge>
      <Badge tone="ink">New</Badge>
    </div>
  );
}
export function States() {
  return (
    <StateSection title="Tones">
      <div className="flex flex-col gap-12">
        <Tones />
        <div className="flex flex-wrap gap-8">
          {(["neutral", "success", "warning", "danger", "info", "outline", "ink"] as const).map((t) => (
            <Badge key={t} tone={t}>
              {t}
            </Badge>
          ))}
        </div>
      </div>
    </StateSection>
  );
}
