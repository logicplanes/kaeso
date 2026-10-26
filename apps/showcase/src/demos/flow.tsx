import { useState } from "react";
import { Button, Flow, ProviderBadge, icons } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

const columns = [
  [{ id: "app", label: "Your app", icon: icons.code }],
  [{ id: "kaeso", label: "Kaeso gateway", icon: icons.gateway, emphasis: true }],
  [
    { id: "openai", label: "gpt-5", leading: <ProviderBadge provider="openai" name="OpenAI" /> },
    { id: "anthropic", label: "claude-sonnet-4-5", leading: <ProviderBadge provider="anthropic" name="Anthropic" /> },
  ],
];
const edges: [string, string][] = [
  ["app", "kaeso"],
  ["kaeso", "openai"],
  ["kaeso", "anthropic"],
];

export const meta = {
  title: "Flow",
  description: "A node-and-connector diagram for the route: app → Kaeso → provider and model.",
  group: "Components",
  entry: "flow",
  import: "Flow",
};
export const examples = [
  { id: "route", title: "Route", fn: "Route" },
  {
    id: "changes",
    title: "A route that changes",
    fn: "Changes",
    description: "With animated, a dot runs once along each connector when the route changes; never on load and never with reduced motion.",
  },
];
export function Changes() {
  const [fallback, setFallback] = useState(false);
  const target = fallback ? "anthropic" : "openai";
  return (
    <div className="flex flex-col items-start gap-16">
      <Flow
        animated
        columns={columns}
        edges={[
          ["app", "kaeso"],
          ["kaeso", target],
        ]}
        aria-label={`Requests go from your app through the Kaeso gateway to ${fallback ? "claude-sonnet-4-5" : "gpt-5"}`}
      />
      <Button variant="secondary" size="sm" onClick={() => setFallback(!fallback)}>
        {fallback ? "Route to gpt-5" : "Route to claude-sonnet-4-5"}
      </Button>
    </div>
  );
}
export function Route() {
  return <Flow columns={columns} edges={edges} aria-label="Requests go from your app through the Kaeso gateway to gpt-5 or claude-sonnet-4-5" />;
}
export function States() {
  return (
    <>
      <StateSection title="Pills and boxes">
        <div className="flex flex-col gap-40">
          <Route />
          <Flow
            node="box"
            columns={[
              [{ id: "a", label: "checkout-web", description: "Key" }],
              [{ id: "b", label: "Gateway", description: "Routes and limits", emphasis: true }],
              [{ id: "c", label: "OpenAI", description: "Provider key …4f2a" }],
            ]}
            edges={[
              ["a", "b"],
              ["b", "c"],
            ]}
            aria-label="Key to gateway to provider"
          />
        </div>
      </StateSection>
      <StateSection title="Narrower than the diagram: it scrolls sideways with a fade and takes focus">
        <div className="w-280">
          <Flow columns={columns} edges={edges} aria-label="Requests go from your app through the Kaeso gateway, 280px" />
        </div>
      </StateSection>
    </>
  );
}
