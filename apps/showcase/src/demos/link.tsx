import { Link } from "@logicplanes/kaeso-ui";
import { Matrix, StateSection } from "~/lib/states";

export const meta = {
  title: "Link",
  description: "Links look like links: text color with a grey underline that takes the text color on hover.",
  group: "Components",
  entry: "link",
  import: "Link",
  keyboard: [["Enter", "Follows the link"]],
  a11y: [
    'External links open in a new tab, add rel="noopener noreferrer" and say so to screen readers.',
    "Router links come from LinkProvider.",
    "The underline is the cue that text is a link, so it stays visible at rest; color alone never marks a link.",
  ],
};
export const examples = [
  { id: "inline", title: "Inline", fn: "Inline" },
  { id: "external", title: "External", fn: "External" },
  { id: "variants", title: "Variants", fn: "Variants" },
  { id: "heading", title: "Heading", fn: "HeadingLink" },
  { id: "cards", title: "Next pages", fn: "Cards" },
];
export function Inline() {
  return (
    <p className="max-w-prose text-body text-fg-secondary">
      Applications authenticate with a key. <Link href="/components/button">Read how keys work</Link> before you create one.
    </p>
  );
}
export function External() {
  return <Link href="https://github.com/logicplanes/kaeso">Read the gateway setup guide</Link>;
}
export function Variants() {
  return (
    <div className="flex items-center gap-24">
      <Link href="#">Inline</Link>
      <Link href="#" variant="quiet">
        Quiet
      </Link>
      <Link href="#" variant="standalone">
        Team overview ›
      </Link>
    </div>
  );
}
export function HeadingLink() {
  return (
    <h2 id="create-a-key" className="text-title font-semibold text-fg">
      <Link href="#create-a-key" variant="heading">
        Create a key
      </Link>
    </h2>
  );
}
export function Cards() {
  return (
    <ul className="grid max-w-640 gap-12 sm:grid-cols-2">
      <li className="flex">
        <Link href="#" variant="card" className="w-full">
          <span className="text-body font-medium">Calling models</span>
          <span className="text-small text-fg-secondary">Call models from your app with the OpenAI and Anthropic SDKs.</span>
        </Link>
      </li>
      <li className="flex">
        <Link href="#" variant="card" className="w-full">
          <span className="text-body font-medium">API keys</span>
          <span className="text-small text-fg-secondary">Limit a key to the models and rates an app needs.</span>
        </Link>
      </li>
    </ul>
  );
}
const LINK_ROWS = [{ label: "inline" }, { label: "quiet" }, { label: "standalone" }, { label: "heading" }, { label: "card" }];

export function States() {
  return (
    <>
      <Matrix
        title="Variants × states"
        rows={LINK_ROWS}
        columns={[
          { label: "Rest", props: {} },
          { label: "Hover", props: { "data-force": "hover" } },
          { label: "Focus", props: { "data-force": "focus" } },
        ]}
        render={(r, p) => (
          <Link href="#" variant={r.label as "inline"} {...p}>
            Provider keys
          </Link>
        )}
      />
      <Matrix
        title="External links"
        rows={LINK_ROWS.map((r) => ({ label: `${r.label} external` }))}
        columns={[
          { label: "Rest", props: {} },
          { label: "Hover", props: { "data-force": "hover" } },
          { label: "Focus", props: { "data-force": "focus" } },
        ]}
        render={(r, p) => (
          <Link href="https://docs.kaeso.example.com" variant={r.label.split(" ")[0] as "inline"} {...p}>
            Provider keys
          </Link>
        )}
      />
      <StateSection title="In running text, wrapping">
        <div className="flex w-320 flex-col gap-16">
          <p className="text-body text-fg-secondary">
            Applications authenticate with a key. <Link href="https://docs.kaeso.example.com/keys">Read how keys, limits and budgets work together</Link> before
            you create one.
          </p>
          <p className="text-body text-fg-secondary">
            Applications authenticate with a key.{" "}
            <Link href="/keys" data-force="focus">
              Read how keys, limits and budgets work together
            </Link>{" "}
            before you create one.
          </p>
        </div>
      </StateSection>
      <StateSection title="Cards with a description">
        <div className="grid w-560 grid-cols-2 gap-12">
          <Link href="#" variant="card">
            <span className="text-body font-medium">Calling models</span>
            <span className="text-small text-fg-secondary">Call models from your app with the OpenAI and Anthropic SDKs.</span>
          </Link>
          <Link href="https://docs.kaeso.example.com/sdk" variant="card" data-force="hover">
            <span className="text-body font-medium">SDK reference</span>
            <span className="text-small text-fg-secondary">Every request and response field, on the documentation site.</span>
          </Link>
        </div>
      </StateSection>
    </>
  );
}
