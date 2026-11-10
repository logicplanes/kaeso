import { useState } from "react";
import { Button, icons } from "@logicplanes/kaeso-ui";
import { Composer, type Attachment } from "@logicplanes/kaeso-ui/playground";
import { prompts } from "~/fixtures/playground";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Composer",
  description: "The prompt box: grows with what is written, then scrolls; Send, which becomes Stop while a response streams.",
  group: "Playground",
  entry: "playground",
  import: "Composer",
  barrel: false,
  keyboard: [
    ["Enter", 'Sends; nothing while a response streams or the prompt is empty (with submitOn="mod-enter", adds a line)'],
    ["Shift Enter", "Adds a line"],
    ["⌘ Enter", "Sends (Ctrl Enter on Windows and Linux)"],
  ],
  a11y: [
    "The text box is named by aria-label (“Prompt” by default) and announces its shortcuts through aria-keyshortcuts; Send and Stop are icon buttons named by their labels, which their tooltips show.",
    "Enter never sends while an input method is composing (Chinese, Japanese, Korean): the Enter that picks a candidate stays the input method's.",
    "On a touch screen Enter adds a line and the Send button sends, as in the messaging apps there; elsewhere the on-screen keyboard's Enter key reads Send (enterkeyhint).",
    "Send and Stop are one button that keeps its place and width, so the focus stays on it when a response starts or ends.",
    "While a response arrives, the text box's description says to stop it to send the next prompt.",
    "A click anywhere in the box, the button row included, puts the cursor in the text box.",
    "Attach files is a button in the row, so files reach the prompt without dragging; dropping and pasting are shortcuts for it. Each waiting file has a remove button named after it.",
  ],
};
export const examples = [
  { id: "basic", title: "Send and stop", fn: "Basic", minHeight: 200 },
  { id: "actions", title: "With actions", description: "Controls at the start of the button row.", fn: "WithActions" },
  {
    id: "attachments",
    title: "With attachments",
    description: "Files picked with Attach files, dropped on the box or pasted wait above the prompt; Send works with files alone.",
    fn: "WithAttachments",
  },
  {
    id: "mod-enter",
    title: "Send with ⌘ Enter",
    description: 'submitOn="mod-enter": Enter adds a line, ⌘ Enter or Ctrl Enter sends. For long prompts written in several lines.',
    fn: "ModEnter",
  },
];
export function Basic() {
  const [streaming, setStreaming] = useState(false);
  return (
    <Composer
      className="w-full max-w-640"
      streaming={streaming}
      onSubmit={() => {
        setStreaming(true);
        setTimeout(() => setStreaming(false), 2400);
      }}
      onStop={() => setStreaming(false)}
    />
  );
}
export function WithActions() {
  return (
    <Composer
      className="w-full max-w-640"
      aria-label="Message all models"
      placeholder="Message 3 models"
      actions={<Button size="sm" variant="ghost" icon={icons.displayOptions} aria-label="Parameters" />}
    />
  );
}
const receipt: Attachment = { id: "receipt", name: "receipt-northwind.svg", type: "image/svg+xml", src: "/demos/receipt.svg" };
const invoice: Attachment = { id: "invoice", name: "invoice-2026-10.pdf", type: "application/pdf" };
export function WithAttachments() {
  const [items, setItems] = useState<Attachment[]>([receipt, invoice]);
  return (
    <Composer
      className="w-full max-w-640"
      placeholder="Ask about the files"
      accept="image/*,application/pdf"
      attachments={items}
      onAttach={(files) =>
        setItems((now) => [
          ...now,
          ...files.map((f) => ({
            id: `${f.name}-${f.size}`,
            name: f.name,
            type: f.type,
            ...(f.type.startsWith("image/") ? { src: URL.createObjectURL(f) } : {}),
          })),
        ])
      }
      onRemoveAttachment={(id) => setItems((now) => now.filter((a) => a.id !== id))}
    />
  );
}
export function ModEnter() {
  return <Composer className="w-full max-w-640" submitOn="mod-enter" placeholder="Write the system prompt" />;
}
function Disabled() {
  return (
    <div className="flex flex-col gap-8">
      <Composer disabled placeholder="Message GPT-5" />
      <p className="px-16 text-small text-fg-secondary">Choose a model to write a prompt.</p>
    </div>
  );
}
export function States() {
  const long = Array.from({ length: 14 }, (_, i) => `${i + 1}. ${prompts[0]}`).join("\n");
  const parameters = (force?: string) => (
    <Button size="sm" variant="ghost" icon={icons.displayOptions} aria-label="Parameters" {...(force ? { "data-force": force } : {})} />
  );
  return (
    <div className="flex w-640 flex-col">
      <StateSection title="Empty">
        <Composer />
      </StateSection>
      <StateSection title="Written">
        <Composer defaultValue={prompts[0]} />
      </StateSection>
      <StateSection title="Hover">
        <Composer data-force="hover" defaultValue={prompts[1]} />
      </StateSection>
      <StateSection title="Focus, by keyboard or click">
        <Composer data-force="focus" defaultValue={prompts[1]} />
      </StateSection>
      <StateSection title="Ten lines, then it scrolls with a fade">
        <Composer defaultValue={long} />
      </StateSection>
      <StateSection title="Sending">
        <Composer loading defaultValue={prompts[0]} />
      </StateSection>
      <StateSection title="Streaming: Stop">
        <Composer streaming defaultValue="" placeholder="Message 3 models" />
      </StateSection>
      <StateSection title="Streaming, the next prompt written">
        <Composer streaming defaultValue={prompts[1]} placeholder="Message 3 models" />
      </StateSection>
      <StateSection title="One line to start (sm)">
        <Composer size="sm" placeholder="Ask a follow-up" />
      </StateSection>
      <StateSection title="sm, grown to three lines">
        <Composer size="sm" defaultValue={`${prompts[0]}\n${prompts[1]}`} />
      </StateSection>
      <StateSection title="Send with ⌘ Enter">
        <Composer submitOn="mod-enter" defaultValue="You write support replies for Northwind." />
      </StateSection>
      <StateSection title="Disabled">
        <Disabled />
      </StateSection>
      <StateSection title="Disabled, with a prompt">
        <Composer disabled defaultValue={prompts[0]} />
      </StateSection>
      <StateSection title="With actions">
        <WithActions />
      </StateSection>
      <StateSection title="With attachments">
        <Composer
          attachments={[receipt, invoice]}
          defaultValue="What did we pay Northwind in October?"
          onAttach={() => undefined}
          onRemoveAttachment={() => undefined}
        />
      </StateSection>
      <StateSection title="Files hovering over it">
        <Composer data-force="drop" attachments={[receipt]} onAttach={() => undefined} onRemoveAttachment={() => undefined} />
      </StateSection>
      <StateSection title="Disabled, with attachments">
        <Composer disabled attachments={[receipt, invoice]} onAttach={() => undefined} onRemoveAttachment={() => undefined} />
      </StateSection>
      <StateSection title="Button row controls on the composer">
        <div className="flex flex-col gap-12">
          {[
            ["rest", undefined],
            ["hover", "hover"],
            ["pressed", "pressed"],
          ].map(([id, force]) => (
            <div key={id} data-cell={`controls-${id}`}>
              <Composer streaming defaultValue="" placeholder="Message 3 models" actions={parameters(force)} />
            </div>
          ))}
        </div>
      </StateSection>
    </div>
  );
}
