import { Status, icons } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Status",
  description: "A status word in its tone's color, with an optional icon. Status is never color alone.",
  group: "Components",
  entry: "status",
  import: "Status",
};
export const examples = [{ id: "tones", title: "Tones", fn: "Tones" }];
export function Tones() {
  return (
    <div className="flex flex-col gap-8">
      <Status tone="success">Active</Status>
      <Status tone="warning">Expiring</Status>
      <Status tone="danger">Down</Status>
      <Status tone="info">Syncing</Status>
      <Status tone="neutral">Archived</Status>
      <Status tone="success">Live</Status>
    </div>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Plain and colored labels">
        <div className="flex gap-48">
          <Tones />
          <div className="flex flex-col gap-8">
            {(["success", "warning", "danger", "info", "neutral"] as const).map((t) => (
              <Status key={t} tone={t} size="sm">
                {t}
              </Status>
            ))}
          </div>
        </div>
      </StateSection>
      <StateSection title="With a mark, small, on a selected row">
        <div className="flex gap-24">
          <Status tone="success" icon={icons.success}>
            Up to date
          </Status>
          <Status tone="danger" icon={icons.error}>
            Down
          </Status>
          <Status tone="success" size="sm" icon={icons.success}>
            Up to date
          </Status>
          <span className="flex h-44 items-center rounded-item bg-fill-2 px-12">
            <Status tone="warning" size="sm">
              Expiring
            </Status>
          </span>
        </div>
      </StateSection>
    </>
  );
}
