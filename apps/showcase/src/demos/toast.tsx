import { Button, Inline, toast, ToastCapsule } from "@logicplanes/kaeso-ui";
import { StateCell, StateMatrix, StateRow, StateSection } from "~/lib/states";

export const meta = {
  title: "Toast",
  description: "An ink capsule at the bottom center: brief, calm, at most three.",
  group: "Components",
  entry: "toast",
  import: "toast",
  keyboard: [["Escape", "Dismisses the focused toast"]],
  a11y: [
    "role=status; errors are alerts.",
    "Five seconds, paused on hover and focus; swipe, Escape or the close button dismisses.",
    "An error, or a toast with Undo, stays until it is dismissed, so nobody has to race it.",
    "F6 moves the focus to the toasts.",
  ],
};
export const examples = [{ id: "basic", title: "Basic", fn: "Basic" }];
export function Basic() {
  return (
    <>
      <Inline gap={8}>
        <Button onClick={() => toast("Key copied")}>Copy key</Button>
        <Button onClick={() => toast.success("Provider saved")}>Save provider</Button>
        <Button onClick={() => toast.error("Couldn't reach the provider.", { description: "Check the provider key, then try again." })}>Test provider</Button>
        <Button onClick={() => toast("Key disabled", { action: { label: "Undo", onClick: () => toast("Key enabled") } })}>Disable key</Button>
        <Button
          onClick={() =>
            void toast.promise(new Promise((r) => setTimeout(r, 1600)), { loading: "Syncing models…", success: "Models synced", error: "Couldn't sync models" })
          }
        >
          Sync models
        </Button>
      </Inline>
    </>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Capsules">
        <div className="flex flex-col items-start gap-16 pt-8 pl-8">
          <ToastCapsule title="Key copied" />
          <ToastCapsule tone="success" title="Provider saved" />
          <ToastCapsule tone="info" title="New models are available" />
          <ToastCapsule tone="loading" title="Syncing models…" />
          <ToastCapsule tone="warning" title="This key expires in 5 days; applications using it stop authenticating with the gateway after that" />
        </div>
      </StateSection>
      <StateSection title="Toasts that stay until dismissed">
        <div className="flex flex-col items-start gap-16 pt-8 pl-8">
          <ToastCapsule tone="error" title="Couldn't reach the provider." description="Check the provider key, then try again." closable />
          <ToastCapsule title="Key disabled" action="Undo" closable />
        </div>
      </StateSection>
      <StateMatrix title="Close button" columns={["Close"]}>
        {(["rest", "hover", "focus"] as const).map((state) => (
          <StateRow key={state} label={state[0]!.toUpperCase() + state.slice(1)}>
            <StateCell id={`close-${state}`}>
              <ToastCapsule tone="error" title="Couldn't copy the key." closable {...(state === "rest" ? {} : { "data-force": state })} />
            </StateCell>
          </StateRow>
        ))}
      </StateMatrix>
    </>
  );
}
