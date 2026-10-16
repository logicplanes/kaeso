import { Button, Inline, toast, ToastCapsule } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Toast",
  description: "An ink capsule at the bottom center: brief, calm, at most three.",
  group: "Components",
  entry: "toast",
  import: "toast",
  keyboard: [["Escape", "Dismisses the focused toast"]],
  a11y: [
    "role=status; errors are alerts.",
    "Five seconds, paused on hover and focus; swipe to dismiss.",
    "Undo lives in the toast, so reversible actions need no confirmation.",
  ],
};
export const examples = [{ id: "basic", title: "Basic", fn: "Basic" }];
export function Basic() {
  return (
    <>
      <Inline gap={8}>
        <Button onClick={() => toast("Key copied")}>Neutral</Button>
        <Button onClick={() => toast.success("Provider saved")}>Success</Button>
        <Button onClick={() => toast.error("Couldn’t reach the provider.", { description: "Check the provider key, then try again." })}>Error</Button>
        <Button onClick={() => toast("Key disabled", { action: { label: "Undo", onClick: () => toast("Key enabled") } })}>With undo</Button>
        <Button
          onClick={() =>
            void toast.promise(new Promise((r) => setTimeout(r, 1600)), { loading: "Syncing models…", success: "Models synced", error: "Couldn’t sync models" })
          }
        >
          Promise
        </Button>
      </Inline>
    </>
  );
}
export function States() {
  return (
    <StateSection title="Capsules">
      <div className="flex flex-col items-start gap-16 pt-8 pl-8">
        <ToastCapsule title="Key copied" />
        <ToastCapsule tone="success" title="Provider saved" />
        <ToastCapsule tone="error" title="Couldn’t reach the provider." description="Check the provider key, then try again." />
        <ToastCapsule tone="loading" title="Syncing models…" />
        <ToastCapsule title="Key disabled" action="Undo" />
        <ToastCapsule title="Key disabled" action="Undo" closable />
        <ToastCapsule tone="warning" title="This key expires in 5 days, and applications using it will stop authenticating with the gateway after that." />
      </div>
    </StateSection>
  );
}
