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
export const examples = [
  { id: "project", title: "Delete a project", fn: "Project" },
  { id: "key", title: "Delete a key", fn: "Key" },
];
export function Project() {
  return (
    <DeleteResource
      kind="project"
      name="Checkout assistant"
      consequence="Its keys stop working at once, and its request logs are deleted. This can't be undone."
      onConfirm={() => new Promise((r) => setTimeout(r, 900))}
      trigger={<Button variant="danger-secondary">Delete project</Button>}
    />
  );
}
export function Key() {
  return (
    <DeleteResource
      kind="key"
      name="checkout-web"
      confirm="simple"
      consequence="Applications using it stop authenticating with the gateway."
      onConfirm={() => new Promise((r) => setTimeout(r, 900))}
      trigger={<Button variant="danger-secondary">Delete key</Button>}
    />
  );
}
export function States() {
  return (
    <StateSection title="Inline">
      <div className="flex flex-col gap-24">
        <DeleteResource
          inline
          kind="project"
          name="Checkout assistant"
          consequence="Its keys stop working at once, and its request logs are deleted. This can't be undone."
          onConfirm={() => undefined}
        />
        <DeleteResource
          inline
          kind="key"
          name="checkout-web"
          confirm="simple"
          consequence="Applications using it stop authenticating with the gateway."
          onConfirm={() => undefined}
        />
        <DeleteResource
          inline
          kind="invitation"
          verb="Revoke"
          name="ana@northwind.example"
          confirm="simple"
          consequence="The link in the email stops working; a new invitation sends a new link."
          onConfirm={() => undefined}
        />
      </div>
    </StateSection>
  );
}
