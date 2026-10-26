import { ConversationView } from "@logicplanes/kaeso-ui";
import { conversation } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "ConversationView",
  description: "A model conversation from a request log: roles, prose, tool calls and results, reasoning, images, refusals.",
  group: "Kaeso",
  entry: "kaeso",
  import: "ConversationView",
  variants: "CONVERSATION",
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
const PIXEL =
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAYAAADED76LAAAAJ0lEQVR4nGNgYGD4z0AEYBxVSFUBRgYGBgYGZgZiASNCIUMNTDUAAKYdAhHzMr5fAAAAAElFTkSuQmCC";
export function States() {
  return (
    <div className="flex w-640 flex-col">
      <StateSection title="Blocks">
        <div className="flex flex-col gap-40">
          <Basic />
          <WithMarkdown />
        </div>
      </StateSection>
      <StateSection title="Inputs, reasoning and a refusal (compact)">
        <ConversationView
          density="compact"
          messages={[
            { role: "developer", content: "Answer in the customer's language. Never quote card numbers." },
            {
              role: "user",
              content: [
                { type: "text", text: "What is in this receipt?" },
                { type: "image", src: PIXEL, alt: "Receipt photo" },
                { type: "image", src: "https://cdn.example.com/uploads/receipt-4471.png" },
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
      </StateSection>
      <StateSection title="Tool results: JSON, an error, nested blocks">
        <ConversationView
          messages={[
            {
              role: "tool",
              content: [
                { type: "tool_result", tool_call_id: "call_8f2k", content: '{"status":"packed","order":"4471","carrier":"DHL","eta":"2026-10-29"}' },
                { type: "tool_result", tool_call_id: "call_9a1c", is_error: true, content: "Shipping service timed out after 10 s." },
                {
                  type: "tool_result",
                  tool_call_id: "call_9b3d",
                  content: [
                    { type: "text", text: "Two attachments were found." },
                    { type: "document", name: "invoice-4471.pdf" },
                  ],
                },
              ],
            },
          ]}
        />
      </StateSection>
      <StateSection title="A long unbroken line">
        <ConversationView
          messages={[
            {
              role: "user",
              content: "https://kaeso.example.com/orgs/northwind/projects/checkout-web/logs?model=gpt-5&status=429&from=2026-10-01T00:00:00Z&to=2026-10-26",
            },
          ]}
        />
      </StateSection>
      <StateSection title="No messages">
        <ConversationView messages={[]} />
      </StateSection>
    </div>
  );
}
