import { Link } from "@logicplanes/kaeso-ui";
import { Matrix } from "~/lib/states";

export const meta = {
  title: "Link",
  description: "Links look like links: text color with a quiet underline that turns solid on hover.",
  group: "Components",
  entry: "link",
  import: "Link",
  keyboard: [["Enter", "Follows the link"]],
  a11y: ['External links open in a new tab, add rel="noopener noreferrer" and say so to screen readers.', "Router links come from LinkProvider."],
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
export function States() {
  return (
    <Matrix
      title="Variants × states"
      rows={[{ label: "inline" }, { label: "quiet" }, { label: "standalone" }, { label: "heading" }, { label: "card" }]}
      columns={[
        { label: "Rest", props: {} },
        { label: "Hover", props: { "data-force": "hover" } },
        { label: "Focus", props: { "data-force": "focus" } },
        { label: "External", props: { external: true } },
      ]}
      render={(r, p) => (
        <Link href="#" variant={r.label as "inline"} {...p}>
          Provider keys
        </Link>
      )}
    />
  );
}
