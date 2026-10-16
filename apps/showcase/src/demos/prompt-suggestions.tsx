import { useState } from "react";
import { Composer, PromptSuggestions } from "@logicplanes/kaeso-ui/playground";
import { StateSection } from "~/lib/states";

const suggestions = ["Apologize for a late order in two sentences", "Explain our refund policy in plain words", "Sort these support emails by topic"];

export const meta = {
  title: "PromptSuggestions",
  description: "Example prompts before a thread starts: up to three quiet lines that take a fill on hover and go once a prompt is sent.",
  group: "Playground",
  entry: "playground",
  import: "PromptSuggestions",
  barrel: false,
  a11y: ["A list named “Example prompts”; each suggestion is a button that reads its prompt."],
};
export const examples = [{ id: "composer", title: "Above the composer", fn: "Basic", minHeight: 280 }];
export function Basic() {
  const [draft, setDraft] = useState("");
  return (
    <div className="flex w-full max-w-640 flex-col gap-12">
      {draft === "" && <PromptSuggestions suggestions={suggestions} onSelect={setDraft} />}
      <Composer value={draft} onValueChange={setDraft} aria-label="Message all models" placeholder="Message 3 models" />
    </div>
  );
}
export function States() {
  const none = () => undefined;
  return (
    <div className="flex w-640 flex-col">
      <StateSection title="Start">
        <PromptSuggestions suggestions={suggestions} onSelect={none} />
      </StateSection>
      <StateSection title="Centered">
        <PromptSuggestions align="center" suggestions={suggestions} onSelect={none} />
      </StateSection>
      <StateSection title="Above the composer">
        <div className="flex flex-col gap-12">
          <PromptSuggestions suggestions={[...suggestions, "A fourth one is left out"]} onSelect={none} />
          <Composer aria-label="Message all models" placeholder="Message 3 models" />
        </div>
      </StateSection>
    </div>
  );
}
