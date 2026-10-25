import { useState } from "react";
import { Button, Dialog, DialogScope, Input, SecretReveal, StepDialog } from "@logicplanes/kaeso-ui";
import { SECRET } from "~/fixtures/kaeso";

export const meta = {
  title: "DialogScope",
  description:
    "Opens a dialog's flow fresh every time: the first step, empty fields, no earlier result or secret. The content stays as it was while the dialog closes.",
  group: "Components",
  entry: "dialog",
  import: "DialogScope",
  a11y: [
    "Each open starts on the first step with the focus on its first field, as a new dialog does.",
    "A secret shown once leaves the page and the component's state when the dialog has closed.",
  ],
};
export const examples = [
  {
    id: "create-key",
    title: "Create a key, then another",
    description: "The key is shown once; closing the dialog forgets it, so the next key starts on the first step.",
    fn: "CreateKey",
    minHeight: 200,
  },
];

function CreateKeyFlow({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [name, setName] = useState("");
  const [created, setCreated] = useState<string | null>(null);
  if (created)
    return (
      <Dialog
        open={open}
        onOpenChange={(o) => !o && onClose()}
        size="lg"
        title="Key created"
        description="Copy the key now: it's shown once."
        footer={<Button onClick={onClose}>Done</Button>}
      >
        <SecretReveal secret={SECRET} name={created} baseUrl="https://kaeso.example.com" />
      </Dialog>
    );
  return (
    <StepDialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title="Create key"
      submitLabel="Create key"
      onSubmit={() => name.trim() && setCreated(name.trim())}
      steps={[
        {
          id: "name",
          title: "What is it for?",
          description: "Shown in request logs and alerts.",
          blockedReason: name.trim() ? undefined : "Enter a name",
          content: <Input aria-label="Name" placeholder="checkout-web" value={name} onChange={(e) => setName(e.target.value)} autoFocus />,
        },
      ]}
    />
  );
}

export function CreateKey() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button variant="primary" onClick={() => setOpen(true)}>
        Create key
      </Button>
      <DialogScope open={open}>
        <CreateKeyFlow open={open} onClose={() => setOpen(false)} />
      </DialogScope>
    </>
  );
}
