import { Button, KeyDisplay, ProviderBadge, ProviderCard, ModelLabel, StatusCode, RequestTimeline } from "@logicplanes/kaeso-ui";
import { keys, models } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "KeyDisplay",
  description: "Kaeso's identities: a redacted key with its state, a model with its provider and capabilities, an HTTP status, a request's timing.",
  group: "Kaeso",
  entry: "kaeso",
  import: "KeyDisplay, ModelLabel, ProviderBadge, ProviderCard, StatusCode, RequestTimeline",
  a11y: [
    "Providers show their mark in the text color, so it follows the theme; others a letter. The name is always next to it or its accessible label.",
    "Capability icons have tooltips and are summarized in an accessible label.",
    "Status codes carry their phrase for screen readers.",
  ],
};
export const examples = [
  { id: "keys", title: "KeyDisplay", fn: "Keys" },
  { id: "models", title: "ModelLabel and ProviderBadge", fn: "Models" },
  { id: "status", title: "StatusCode and RequestTimeline", fn: "Status" },
  { id: "providers", title: "ProviderCard", fn: "Providers" },
];
export function Providers() {
  return (
    <div className="grid w-full grid-cols-1 gap-16 md:grid-cols-2">
      <ProviderCard
        provider="openai"
        connected
        status="The organization's key"
        keyLabel="Production …4f2a"
        detail="Last used 2 min ago"
        actions={<Button size="sm">Use another key here</Button>}
      />
      <ProviderCard
        provider="anthropic"
        connected={false}
        status="No key yet: requests to Anthropic fail"
        actions={
          <Button size="sm" variant="primary">
            Connect Anthropic
          </Button>
        }
      />
    </div>
  );
}
export function Keys() {
  return (
    <div className="flex flex-col gap-12">
      {keys.slice(0, 4).map((k) => (
        <KeyDisplay key={k.id} redacted={k.redacted} state={k.state} access={k.access} />
      ))}
      <KeyDisplay redacted="sk-kso-Rt82Vn…3c1d" access="both" />
    </div>
  );
}
export function Models() {
  return (
    <div className="flex flex-col gap-12">
      {models.map((m) => (
        <ModelLabel key={m.id} provider={m.provider} model={m.model} name={m.name} capabilities={m.capabilities} />
      ))}
      <div className="flex gap-12">
        <ProviderBadge provider="openai" withName />
        <ProviderBadge provider="anthropic" withName />
      </div>
    </div>
  );
}
export function Status() {
  return (
    <div className="flex flex-col gap-16">
      <div className="flex gap-16">
        <StatusCode code={200} />
        <StatusCode code={429} />
        <StatusCode code={502} showText />
      </div>
      <RequestTimeline ttfbMs={412} totalMs={1840} />
    </div>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Keys">
        <Keys />
      </StateSection>
      <StateSection title="Models">
        <Models />
      </StateSection>
      <StateSection title="Status">
        <div className="flex w-full flex-col gap-24">
          <Status />
          <Providers />
        </div>
      </StateSection>
      <StateSection title="RequestTimeline at 160px, 240px and full: each time over its label, the same height whatever the times">
        <div className="flex flex-col gap-24">
          <div className="flex flex-wrap items-start gap-24">
            <RequestTimeline ttfbMs={412} totalMs={1840} width={160} />
            <RequestTimeline ttfbMs={10054} totalMs={125532} width={160} />
            <RequestTimeline ttfbMs={412} totalMs={1840} />
          </div>
          <div className="w-full max-w-480">
            <RequestTimeline ttfbMs={1054} totalMs={15532} width="full" />
          </div>
        </div>
      </StateSection>
    </>
  );
}
