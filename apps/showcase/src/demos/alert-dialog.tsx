import { AlertDialog, Button } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "AlertDialog",
  description: "Confirms an irreversible action: names it, says the consequence, can ask for typed confirmation.",
  group: "Components",
  entry: "dialog",
  import: "AlertDialog",
  keyboard: [
    ["Escape", "Cancels"],
    ["Enter", "Confirms once the typed text matches"],
  ],
  a11y: ["role=alertdialog; focus starts on Cancel.", "The confirm names the verb and object (“Delete key”)."],
};
export const examples = [
  { id: "delete", title: "Delete a key", fn: "Delete" },
  { id: "typed", title: "Typed confirmation", fn: "Typed" },
];
export function Delete() {
  return (
    <AlertDialog
      trigger={<Button variant="danger-secondary">Delete key</Button>}
      title="Delete this key?"
      description="Applications using checkout-web will stop authenticating with the gateway."
      confirmLabel="Delete key"
      onConfirm={() => new Promise((r) => setTimeout(r, 900))}
    />
  );
}
export function Typed() {
  return (
    <AlertDialog
      trigger={<Button variant="danger-secondary">Delete organization</Button>}
      title="Delete Northwind Labs?"
      description="Its projects, keys and logs are deleted after 30 days. Until then an instance admin can restore it."
      confirmLabel="Delete organization"
      confirmText="Northwind Labs"
      onConfirm={() => undefined}
    />
  );
}
export function States() {
  return (
    <StateSection title="Inline">
      <div className="flex flex-col gap-24">
        <AlertDialog
          inline
          title="Delete this key?"
          description="Applications using checkout-web will stop authenticating with the gateway."
          confirmLabel="Delete key"
          onConfirm={() => undefined}
        />
        <AlertDialog
          inline
          title="Delete Northwind Labs?"
          description="Its projects, keys and logs are deleted after 30 days."
          confirmLabel="Delete organization"
          confirmText="Northwind Labs"
          onConfirm={() => undefined}
        />
        <AlertDialog
          inline
          tone="warning"
          title="Disable this key?"
          description="Requests with it fail until you enable it again."
          confirmLabel="Disable key"
          loading
          onConfirm={() => undefined}
        />
      </div>
    </StateSection>
  );
}
