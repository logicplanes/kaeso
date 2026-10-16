import { Sankey } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

const nodes = [
  { id: "checkout-web", label: "checkout-web" },
  { id: "checkout-worker", label: "checkout-worker" },
  { id: "ci-runner", label: "ci-runner" },
  { id: "gpt-5", label: "gpt-5" },
  { id: "claude-sonnet-4-5", label: "claude-sonnet-4-5" },
  { id: "gpt-5-mini", label: "gpt-5-mini" },
  { id: "openai", label: "OpenAI" },
  { id: "anthropic", label: "Anthropic" },
];
const links = [
  { source: "checkout-web", target: "gpt-5", value: 4200 },
  { source: "checkout-web", target: "claude-sonnet-4-5", value: 2100 },
  { source: "checkout-worker", target: "gpt-5-mini", value: 3600 },
  { source: "checkout-worker", target: "claude-sonnet-4-5", value: 900 },
  { source: "ci-runner", target: "gpt-5-mini", value: 700 },
  { source: "gpt-5", target: "openai", value: 4200 },
  { source: "gpt-5-mini", target: "openai", value: 4300 },
  { source: "claude-sonnet-4-5", target: "anthropic", value: 3000 },
];

export const meta = {
  title: "Sankey",
  description: "Requests from keys to models to providers; hover a node to follow its path.",
  group: "Charts",
  entry: "charts",
  import: "Sankey",
};
export const examples = [{ id: "routes", title: "Routes", fn: "Routes", minHeight: 380 }];
export function Routes() {
  return (
    <div className="w-full">
      <Sankey aria-label="Requests from keys to models to providers" nodes={nodes} links={links} />
    </div>
  );
}
export function States() {
  return (
    <StateSection title="Routes">
      <div className="w-960">
        <Routes />
      </div>
    </StateSection>
  );
}
