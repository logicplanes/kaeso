import { Sankey } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";
import { AsTable, Labelled } from "~/lib/chart-states";

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
    <>
      <StateSection title="Routes">
        <div className="w-640">
          <Routes />
        </div>
      </StateSection>
      <StateSection title="Narrow: values drop, then names shorten with the full name on hover">
        <div className="flex flex-col gap-40">
          <div className="w-400">
            <Sankey aria-label="Requests from keys to models to providers, 400px" nodes={nodes} links={links} height={280} />
          </div>
          <div className="w-280">
            <Sankey aria-label="Requests from keys to models to providers, 280px" nodes={nodes} links={links} height={280} />
          </div>
        </div>
      </StateSection>
      <StateSection title="Empty, loading, error">
        <div className="grid w-640 grid-cols-2 gap-x-24 gap-y-40">
          <Labelled title="No requests">
            <Sankey aria-label="Requests, none" nodes={nodes} links={[]} height={200} />
          </Labelled>
          <Labelled title="Loading">
            <Sankey aria-label="Requests, loading" nodes={[]} links={[]} height={200} loading />
          </Labelled>
          <Labelled title="Error">
            <Sankey aria-label="Requests, failed" nodes={nodes} links={links} height={200} error onRetry={() => undefined} />
          </Labelled>
          <Labelled title="As a table">
            <AsTable>
              <Sankey aria-label="Requests, as a table" nodes={nodes} links={links} height={200} />
            </AsTable>
          </Labelled>
        </div>
      </StateSection>
    </>
  );
}
