import { useState } from "react";
import { Button, Checkbox, CheckboxGroup, Dialog, Input, Segmented, Select, StepDialog, toast, type DialogStep } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Dialog",
  description:
    "A modal dialog: 440, 560 or 720 wide, radius 20; title, description, body, actions. ⌘Enter submits. Longer forms use StepDialog: one question per step.",
  group: "Components",
  entry: "dialog",
  import: "Dialog",
  keyboard: [
    ["Escape", "Closes and returns focus to the trigger"],
    ["Tab", "Moves within the dialog"],
    ["mod Enter", "Submits"],
  ],
  a11y: [
    "Focus moves into the dialog and returns to the trigger when it closes.",
    "The title names the dialog; the description is its accessible description.",
    "The footer line appears only while content scrolls under it.",
    "In steps, Continue and Back move focus to the new step's first field, or its heading when it has none, and the step's number and question are announced.",
    "In steps, the dialog takes each step's height and keeps its top; with reduced motion the step and the height change at once.",
  ],
};
export const examples = [
  { id: "form", title: "Create a key", fn: "Form" },
  { id: "steps", title: "In steps, one question at a time", fn: "Steps" },
  { id: "step-heights", title: "Steps of different heights", fn: "StepHeights" },
  { id: "sizes", title: "Sizes", fn: "Sizes" },
];
const expiry = [
  { value: "30", label: "In 30 days" },
  { value: "90", label: "In 90 days" },
  { value: "never", label: "Never" },
];
export function Steps() {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  return (
    <>
      <Button variant="primary" onClick={() => setOpen(true)}>
        Create key
      </Button>
      <StepDialog
        open={open}
        onOpenChange={setOpen}
        title="Create key"
        submitLabel="Create key"
        onSubmit={() => {
          setOpen(false);
          toast.success(`Key ${name} created`);
        }}
        steps={[
          {
            id: "name",
            title: "What is it for?",
            description: "Shown in request logs and alerts.",
            blockedReason: name.trim() ? undefined : "Enter a name",
            content: <Input aria-label="Name" placeholder="checkout-web" value={name} onChange={(e) => setName(e.target.value)} autoFocus />,
          },
          {
            id: "expiry",
            title: "When does it expire?",
            description: "Choose the shortest time the key is needed.",
            content: <Select aria-label="Expires" options={expiry} defaultValue="90" fullWidth />,
          },
        ]}
      />
    </>
  );
}
const periods = [
  { value: "day", label: "Each day" },
  { value: "month", label: "Each month" },
];
const members = [
  { value: "ana", label: "Ana Ruiz" },
  { value: "deploy-bot", label: "deploy-bot (service account)" },
  { value: "jon", label: "Jon Park" },
];
const models = [
  "claude-opus-5-5",
  "claude-sonnet-5-5",
  "claude-haiku-5-5",
  "claude-sonnet-4-5",
  "claude-haiku-4-5",
  "claude-opus-4-1",
  "gpt-6-sol",
  "gpt-6-luna",
  "gpt-5.6",
  "gpt-5.4-mini",
  "gpt-5.4-nano",
  "gpt-5",
  "gpt-5-mini",
  "gpt-4.1-nano",
  "gemini-3-pro",
  "gemini-3-flash",
  "gemini-2.5-pro",
  "gemini-2.5-flash",
  "mistral-large-3",
  "mistral-medium-3",
  "codestral-2",
  "llama-4-maverick",
  "llama-4-scout",
  "deepseek-v4",
];
function limitSteps(level: string, setLevel: (v: string) => void): DialogStep[] {
  return [
    {
      id: "who",
      title: "Who does it apply to?",
      description: "A request passes its key's, its member's and the project's limits.",
      content: (
        <div className="flex flex-col gap-12">
          <Segmented
            aria-label="Applies to"
            value={level}
            onValueChange={setLevel}
            options={[
              { value: "project", label: "Project" },
              { value: "member", label: "Member" },
            ]}
            fullWidth
          />
          {level === "member" && <Select aria-label="Member or service account" placeholder="Choose a member" options={members} fullWidth />}
        </div>
      ),
    },
    {
      id: "limit",
      title: "What's the limit?",
      description: "Budgets refuse requests once used up; an alert comes first.",
      content: (
        <div className="flex flex-col gap-20">
          <Segmented
            aria-label="Kind"
            defaultValue="budget"
            options={[
              { value: "budget", label: "Spend budget" },
              { value: "rate", label: "Rate limit" },
            ]}
            fullWidth
          />
          <Input label="Amount" prefix="$" defaultValue="500" inputMode="decimal" />
          <Select label="Resets" options={periods} defaultValue="month" fullWidth />
          <Input label="Alert at" description="Owners get an email when spend passes it." suffix="%" defaultValue="80" inputMode="numeric" />
        </div>
      ),
    },
    {
      id: "models",
      title: "Which models does it cover?",
      description: "Requests to other models pass without it.",
      content: (
        <CheckboxGroup aria-label="Models" defaultValue={models.slice(0, 3)} className="flex flex-col">
          {models.map((m) => (
            <Checkbox key={m} value={m} label={<span className="mono">{m}</span>} />
          ))}
        </CheckboxGroup>
      ),
    },
  ];
}
export function StepHeights() {
  const [open, setOpen] = useState(false);
  const [level, setLevel] = useState("project");
  return (
    <>
      <Button variant="primary" onClick={() => setOpen(true)}>
        Add limit
      </Button>
      <StepDialog
        open={open}
        onOpenChange={setOpen}
        title="Add limit"
        size="md"
        submitLabel="Add limit"
        onSubmit={() => {
          setOpen(false);
          toast.success("Limit added");
        }}
        steps={limitSteps(level, setLevel)}
      />
    </>
  );
}
export function Form() {
  const [open, setOpen] = useState(false);
  const create = () => {
    setOpen(false);
    toast.success("Key created");
  };
  return (
    <>
      <Dialog
        open={open}
        onOpenChange={setOpen}
        trigger={<Button variant="primary">Create key</Button>}
        title="Create key"
        description="Applications use this key to call the gateway."
        onSubmit={create}
        footer={
          <>
            <Dialog.Close>
              <Button>Cancel</Button>
            </Dialog.Close>
            <Button variant="primary" onClick={create} shortcut={["mod", "enter"]}>
              Create key
            </Button>
          </>
        }
      >
        <div className="flex flex-col gap-20">
          <Input label="Name" placeholder="checkout-web" autoFocus />
          <Select
            label="Expires"
            options={[
              { value: "never", label: "Never" },
              { value: "90", label: "In 90 days" },
            ]}
            defaultValue="90"
            fullWidth
          />
        </div>
      </Dialog>
    </>
  );
}
export function Sizes() {
  return (
    <div className="flex gap-12">
      {(["sm", "md", "lg"] as const).map((s) => (
        <Dialog
          key={s}
          size={s}
          trigger={<Button>{s}</Button>}
          title={`A ${s} dialog`}
          description="Sizes are 440, 560 and 720 pixels."
          footer={
            <Dialog.Close>
              <Button>Close</Button>
            </Dialog.Close>
          }
        />
      ))}
    </div>
  );
}
export function States() {
  return (
    <StateSection title="Inline frames">
      <div className="flex flex-col gap-24">
        <Dialog
          inline
          size="md"
          title="Create key"
          description="Applications use this key to call the gateway."
          footer={
            <>
              <Button>Cancel</Button>
              <Button variant="primary">Create key</Button>
            </>
          }
        >
          <div className="flex flex-col gap-20">
            <Input label="Name" defaultValue="checkout-web" />
            <Select label="Expires" options={[{ value: "90", label: "In 90 days" }]} defaultValue="90" fullWidth />
          </div>
        </Dialog>
        <StepDialog
          inline
          title="Create key"
          submitLabel="Create key"
          onSubmit={() => undefined}
          steps={[
            {
              id: "name",
              title: "What is it for?",
              description: "Shown in request logs and alerts.",
              content: <Input aria-label="Name" defaultValue="checkout-web" />,
            },
            { id: "expiry", title: "When does it expire?", content: <Select aria-label="Expires" options={expiry} defaultValue="90" fullWidth /> },
          ]}
        />
        <StepDialog inline size="md" title="Add limit" submitLabel="Add limit" onSubmit={() => undefined} steps={limitSteps("project", () => undefined)} />
        <StepDialog
          inline
          size="md"
          title="Add limit"
          submitLabel="Add limit"
          onSubmit={() => undefined}
          defaultStep={1}
          steps={limitSteps("project", () => undefined)}
        />
        <Dialog
          inline
          size="sm"
          title="Leave Support triage?"
          description="You will lose access to its keys and logs."
          footer={
            <>
              <Button>Cancel</Button>
              <Button variant="primary">Leave project</Button>
            </>
          }
        />
      </div>
    </StateSection>
  );
}
