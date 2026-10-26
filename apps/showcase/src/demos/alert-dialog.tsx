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
  a11y: [
    "role=alertdialog. The focus starts on the typed field when there is one; otherwise on Cancel, or on the confirm for an info confirmation.",
    "The confirm names the verb and object (“Delete key”).",
    "The Enter that finishes typing in an input method never confirms.",
  ],
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
        <AlertDialog
          inline
          tone="info"
          title="Send the invitation again?"
          description="ana@northwind.example gets a new link; the old one stops working."
          confirmLabel="Send invitation"
          onConfirm={() => undefined}
        />
        <AlertDialog
          inline
          title="Revoke sk-kso-AbC123dEf456gHi789jKl012mNo345pQr678?"
          description="Applications using sk-kso-AbC123dEf456gHi789jKl012mNo345pQr678 stop authenticating with the gateway at once."
          confirmLabel="Revoke key"
          onConfirm={() => undefined}
        />
      </div>
    </StateSection>
  );
}
