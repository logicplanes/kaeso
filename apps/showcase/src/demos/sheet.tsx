import { Button, Sheet, KeyValue, StatusCode, RequestTimeline, IconButton, icons } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Sheet",
  description: "A panel from the edge that keeps the list behind it readable: the request inspector, details.",
  group: "Components",
  entry: "sheet",
  import: "Sheet",
  keyboard: [["Escape", "Closes"]],
  a11y: ["A dialog; the title names it, the subtitle carries the id.", "Apps make it URL-addressable so links can open it."],
};
const body = (
  <div className="flex flex-col gap-24">
    <RequestTimeline ttfbMs={412} totalMs={1840} />
    <KeyValue
      items={[
        { label: "Status", value: <StatusCode code={200} showText /> },
        { label: "Model", value: "claude-sonnet-4-5", mono: true, copy: "claude-sonnet-4-5" },
        { label: "Key", value: "checkout-web" },
        { label: "Tokens", value: "4,812 in · 612 out" },
        { label: "Cost", value: "$0.023616" },
      ]}
    />
  </div>
);
export const examples = [{ id: "inspector", title: "Request inspector", fn: "Inspector" }];
export function Inspector() {
  return (
    <Sheet
      trigger={<Button>Open request</Button>}
      title="POST /v1/messages"
      subtitle="req_01JAR4X2KQ"
      size="md"
      modal={false}
      actions={<IconButton icon={icons.copy} aria-label="Copy request id" />}
    >
      {body}
    </Sheet>
  );
}
export function States() {
  return (
    <StateSection title="Inline">
      <Sheet inline title="POST /v1/messages" subtitle="req_01JAR4X2KQ" size="sm" actions={<IconButton icon={icons.copy} aria-label="Copy request id" />}>
        {body}
      </Sheet>
    </StateSection>
  );
}
