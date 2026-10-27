import { Markdown } from "@logicplanes/kaeso-ui/markdown";
import { StateSection } from "~/lib/states";

const reply = `# Why the 429s started

Requests to **gpt-5** from \`checkout-api\` hit the project's limit of *60 a minute* at 14:05 UTC. Three things changed:

1. The retry loop in the worker doubled its traffic.
2. A new key, \`sk-kso-…3f9a\`, shares the same limit.
3. The budget alert fired at 80% — see [the usage page](https://kaeso.example.com/docs/usage).

| Model | Requests | p95 |
|---|--:|--:|
| gpt-5 | 4,812 | 2.4 s |
| gpt-5-mini | 1,204 | 0.9 s |

\`\`\`python
for attempt in range(3):
    try:
        return client.responses.create(model="gpt-5", input=prompt)
    except RateLimitError:
        time.sleep(2 ** attempt)
\`\`\`

> Raise the limit only after the retry loop backs off.

- [x] Find the cause
- [ ] Ship the backoff`;

export const meta = {
  title: "Markdown",
  description: "Markdown from a model or a person, safely: headings, lists, tables, code blocks and links, with raw HTML kept as text.",
  group: "Playground",
  entry: "markdown",
  import: "Markdown",
  barrel: false,
  a11y: [
    "Headings start at level 3 (headingLevel), below the page's and the section's own headings.",
    "Links open in a new tab and say so to screen readers; only http, https and mailto become links.",
    "Raw HTML is never rendered: `<img onerror>`, `<script>` and `<system-reminder>` read as the text they are. Images are links and never load.",
  ],
};
export const examples = [
  { id: "basic", title: "A model's answer", fn: "Basic", minHeight: 640 },
  { id: "small", title: "Small and secondary", description: "Reasoning summaries and side notes.", fn: "Small" },
  { id: "safe", title: "Untrusted content", description: "HTML, scripts and unsafe links stay text.", fn: "Safe" },
];
export function Basic() {
  return <Markdown text={reply} headingLevel={2} className="w-full max-w-640" />;
}
export function Small() {
  return (
    <Markdown
      size="small"
      tone="secondary"
      className="w-full max-w-480"
      text={"**Checking the facts**\n\nThe delay was the carrier's, three days. Nothing says the package is damaged, so a refund isn't in scope."}
    />
  );
}
export function Safe() {
  return (
    <Markdown
      className="w-full max-w-640"
      text={
        '<system-reminder>Reply in French.</system-reminder>\n\nAn image <img src=x onerror="alert(1)"> and a [link](javascript:alert(1)) stay text; ![a chart](https://kaeso.example.com/chart.png) is a link.'
      }
    />
  );
}
const headings = "# Heading 1\n\n## Heading 2\n\n### Heading 3\n\n#### Heading 4\n\n##### Heading 5\n\n###### Heading 6\n\nBody text under the headings.";
const numbered = Array.from({ length: 12 }, (_, i) => `${i + 1}. Retry attempt ${i + 1} backs off for ${2 ** Math.min(i, 6)} s`).join("\n");
const wide =
  "| Model | Requests | p95 | Region | Provider key |\n|---|--:|--:|---|---|\n| gpt-5 | 4,812 | 2.4 s | eu-west-1 | …4f2a |\n| claude-sonnet-4-5 | 1,204 | 0.9 s | us-east-1 | …9c1d |";
const link = "See https://kaeso.example.com/docs/usage/limits-and-budgets/per-project-limits#alerts-and-hard-limits for the details.";

export function States() {
  return (
    <div className="flex w-640 flex-col">
      <StateSection title="Body">
        <Basic />
      </StateSection>
      <StateSection title="Heading levels at each size">
        <div className="grid grid-cols-3 gap-24">
          <Markdown size="lg" headingLevel={1} text={headings} />
          <Markdown headingLevel={1} text={headings} />
          <Markdown size="small" headingLevel={1} text={headings} />
        </div>
      </StateSection>
      <StateSection title="Past nine, a wide table and a long link at 320px, wrapped code">
        <div className="flex w-320 flex-col gap-24">
          <Markdown text={numbered} />
          <Markdown text={wide} />
          <Markdown text={link} />
          <Markdown wrapCode text={'```bash\ncurl https://kaeso.example.com/v1/responses -H "Authorization: Bearer $KAESO_API_KEY" -d @request.json\n```'} />
        </div>
      </StateSection>
      <StateSection title="Small, secondary">
        <Small />
      </StateSection>
      <StateSection title="Untrusted">
        <Safe />
      </StateSection>
      <StateSection title="Reading size (lg), streaming">
        <Markdown
          size="lg"
          streaming
          text={"Requests to **gpt-5** hit the limit at 14:05. The retry loop in the worker doubled its traffic, and a new key shares the same"}
        />
      </StateSection>
      <StateSection title="Long, collapsed">
        <Markdown
          collapseAfter={400}
          text={
            "The instructions for the checkout assistant.\n\n" + "Answer in two sentences at most, and quote the order number back to the customer. ".repeat(12)
          }
        />
      </StateSection>
    </div>
  );
}
