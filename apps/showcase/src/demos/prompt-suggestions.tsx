import { useRef, useState } from "react";
import { Composer, PromptSuggestions } from "@logicplanes/kaeso-ui/playground";
import { StateSection } from "~/lib/states";

const suggestions = ["Apologize for a late order in two sentences", "Explain our refund policy in plain words", "Sort these support emails by topic"];

export const meta = {
  title: "PromptSuggestions",
  description: "Example prompts before a thread starts: up to three quiet rows that take a fill on hover and go once a prompt is sent.",
  group: "Playground",
  entry: "playground",
  import: "PromptSuggestions",
  barrel: false,
  a11y: [
    "A list named “Example prompts”; each suggestion is a button that reads its prompt.",
    "Choosing one puts it in the composer and moves the focus there.",
  ],
};
export const examples = [{ id: "composer", title: "Above the composer", fn: "Basic", minHeight: 280 }];
export function Basic() {
  const [draft, setDraft] = useState("");
  const composer = useRef<HTMLTextAreaElement | null>(null);
  return (
    <div className="flex w-full max-w-640 flex-col gap-12">
      {draft === "" && (
        <PromptSuggestions
          suggestions={suggestions}
          onSelect={(p) => {
            setDraft(p);
            composer.current?.focus();
          }}
        />
      )}
      <Composer ref={composer} value={draft} onValueChange={setDraft} aria-label="Message all models" placeholder="Message 3 models" />
    </div>
  );
}
export function States() {
  const none = () => undefined;
  const long = "Summarize the last 30 days of request logs for checkout-web by model, provider and status, then flag anything over budget";
  return (
    <div className="flex w-640 flex-col">
      <StateSection title="Start">
        <PromptSuggestions suggestions={suggestions} onSelect={none} />
      </StateSection>
      <StateSection title="Hover, focus, pressed">
        <div className="flex flex-col gap-8">
          {[
            ["rest", undefined],
            ["hover", "hover"],
            ["focus", "focus"],
            ["pressed", "pressed"],
          ].map(([id, force]) => (
            <div key={id} data-cell={`row-${id}`}>
              <PromptSuggestions
                aria-label={`Example prompts, ${id}`}
                suggestions={[suggestions[0]!]}
                onSelect={none}
                {...(force ? { "data-force": force } : {})}
              />
            </div>
          ))}
        </div>
      </StateSection>
      <StateSection title="One suggestion">
        <PromptSuggestions suggestions={[suggestions[1]!]} onSelect={none} />
      </StateSection>
      <StateSection title="Too long for the row: the full prompt in a tooltip">
        <div className="w-400">
          <PromptSuggestions suggestions={[long, suggestions[2]!]} onSelect={none} />
        </div>
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
