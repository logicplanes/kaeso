import { useState } from "react";
import { Button, icons } from "@logicplanes/kaeso-ui";
import { Composer } from "@logicplanes/kaeso-ui/playground";
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
    "The text box is named by aria-label (“Prompt” by default) and announces its shortcuts through aria-keyshortcuts; no hint is drawn.",
    "Enter never sends while an input method is composing (Chinese, Japanese, Korean): the Enter that picks a candidate stays the input method's.",
    "On a touch screen Enter adds a line and the Send button sends, as in the messaging apps there; elsewhere the on-screen keyboard's Enter key reads Send (enterkeyhint).",
    "Send and Stop are one button that keeps its place and width, so the focus stays on it when a response starts or ends.",
  ],
};
export const examples = [
  { id: "basic", title: "Send and stop", fn: "Basic", minHeight: 200 },
  { id: "actions", title: "With actions", description: "Controls at the start of the button row.", fn: "WithActions" },
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
      actions={
        <Button size="sm" variant="ghost" leadingIcon={icons.displayOptions}>
          Parameters
        </Button>
      }
    />
  );
}
export function ModEnter() {
  return <Composer className="w-full max-w-640" submitOn="mod-enter" placeholder="Write the system prompt" />;
}
export function States() {
  const long = Array.from({ length: 14 }, (_, i) => `${i + 1}. ${prompts[0]}`).join("\n");
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
      <StateSection title="Focus">
        <Composer data-force="focus" defaultValue={prompts[1]} />
      </StateSection>
      <StateSection title="Ten lines, then it scrolls">
        <Composer defaultValue={long} />
      </StateSection>
      <StateSection title="Sending">
        <Composer loading defaultValue={prompts[0]} />
      </StateSection>
      <StateSection title="Streaming: Stop">
        <Composer streaming defaultValue="" placeholder="Message 3 models" />
      </StateSection>
      <StateSection title="One line to start (sm)">
        <Composer size="sm" placeholder="Ask a follow-up" />
      </StateSection>
      <StateSection title="Disabled">
        <Composer disabled placeholder="Choose a model to write a prompt" />
      </StateSection>
      <StateSection title="With actions">
        <WithActions />
      </StateSection>
    </div>
  );
}
