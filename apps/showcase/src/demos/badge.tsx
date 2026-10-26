import { Badge, Status, Tag } from "@logicplanes/kaeso-ui";
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
const TONES = ["neutral", "success", "warning", "danger", "info", "outline", "ink"] as const;
const WORD = { neutral: "Inference", success: "Active", warning: "Expiring", danger: "Failed", info: "Syncing", outline: "Platform", ink: "New" };
const SURFACES = [
  { label: "Panel", className: "bg-panel" },
  { label: "Row hover", className: "bg-fill-1 controls-on-fill" },
  { label: "Selected row", className: "bg-fill-2 controls-on-fill" },
  { label: "Card", className: "bg-raised shadow-xs" },
  { label: "Selected in a card", className: "bg-raised-pressed controls-on-fill" },
];
export function States() {
  return (
    <>
      <StateSection title="Every tone on every surface a badge sits on">
        <div className="flex flex-col gap-8">
          {SURFACES.map((surface) => (
            <div key={surface.label} className="flex flex-col gap-4">
              <span className="text-caption font-medium text-fg-tertiary">{surface.label}</span>
              <div className={`flex h-44 items-center gap-8 rounded-item px-12 ${surface.className}`}>
                {TONES.map((t) => (
                  <Badge key={t} tone={t}>
                    {WORD[t]}
                  </Badge>
                ))}
              </div>
            </div>
          ))}
        </div>
      </StateSection>
      <StateSection title="Beside a tag and a status, and a long label in a narrow column">
        <div className="flex flex-col gap-12">
          <div className="flex items-center gap-8">
            <Badge>Owner</Badge>
            <Tag size="sm">gpt-5</Tag>
            <Status tone="success">Healthy</Status>
          </div>
          <div className="flex w-120">
            <Badge>Platform administrator</Badge>
          </div>
        </div>
      </StateSection>
    </>
  );
}
