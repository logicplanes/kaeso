import { DeleteResource, Button } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "DeleteResource",
  description: "The destructive flow: the object and the consequence, typed confirmation, a danger button.",
  group: "Blocks",
  entry: "delete-resource",
  import: "DeleteResource",
  source: "blocks/delete-resource",
};
export const examples = [{ id: "project", title: "Archive a project", fn: "Project" }];
export function Project() {
  return (
    <DeleteResource
      kind="project"
      verb="Archive"
      name="Checkout assistant"
      consequence="Its keys stop working. Logs stay for 30 days, then they are deleted."
      onConfirm={() => new Promise((r) => setTimeout(r, 900))}
      trigger={<Button variant="danger-secondary">Archive project</Button>}
    />
  );
}
export function States() {
  return (
    <StateSection title="Inline">
      <DeleteResource
        inline
        kind="project"
        verb="Archive"
        name="Checkout assistant"
        consequence="Its keys stop working. Logs stay for 30 days, then they are deleted."
        onConfirm={() => undefined}
      />
    </StateSection>
  );
}
