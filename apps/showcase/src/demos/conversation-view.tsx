import { ConversationView } from "@logicplanes/kaeso-ui";
import { conversation } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "ConversationView",
  description: "A model conversation from a request log: roles, prose, tool calls and results, reasoning, images, refusals.",
  group: "Kaeso",
  entry: "kaeso",
  import: "ConversationView",
};
export const examples = [
  { id: "basic", title: "Checkout conversation", fn: "Basic", minHeight: 640 },
  {
    id: "markdown",
    title: "Text as Markdown",
    description: "markdown draws text, reasoning and prose tool results as Markdown, safely; the renderer loads the first time it is needed.",
    fn: "WithMarkdown",
    minHeight: 360,
  },
];
export function Basic() {
  return <ConversationView messages={conversation} className="w-640" />;
}
export function WithMarkdown() {
  return (
    <ConversationView
      markdown
      className="w-full max-w-640"
      messages={[
        { role: "user", content: "Which models failed most today? Answer with a table.\n\n<system-reminder>Keep it short.</system-reminder>" },
        {
          role: "assistant",
          content:
            "Two did:\n\n| Model | Errors | Rate |\n|---|--:|--:|\n| gpt-5 | 41 | 0.9% |\n| claude-haiku-4-5 | 12 | 0.3% |\n\nMost were **429s** from the project's limit. See the [request logs](https://kaeso.example.com/docs/logs).",
        },
      ]}
    />
  );
}
export function States() {
  return (
    <StateSection title="Blocks">
      <div className="flex w-640 flex-col gap-40">
        <Basic />
        <WithMarkdown />
        <ConversationView
          density="compact"
          messages={[
            {
              role: "user",
              content: [
                { type: "text", text: "What is in this receipt?" },
                { type: "image", media_type: "image/png" },
                { type: "document", name: "receipt-4471.pdf" },
              ],
            },
            {
              role: "assistant",
              content: [
                { type: "reasoning", text: "", redacted: true },
                { type: "refusal", text: "I can't read payment card numbers from images." },
              ],
            },
          ]}
        />
      </div>
    </StateSection>
  );
}
