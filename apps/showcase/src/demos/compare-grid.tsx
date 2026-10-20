import { CompareGrid, ModelPicker, ResponseMetrics, ResponseView, Thread, UserMessage } from "@logicplanes/kaeso-ui/playground";
import { playgroundModels } from "~/fixtures/playground";
import { providers } from "~/fixtures/kaeso";
import { PlaygroundWorkspace, answered, type Column } from "./_playground-workspace";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "CompareGrid",
  description:
    "Models side by side: one to four equal columns, each with a header, a thread that scrolls on its own and a footer. One column per screen on a phone.",
  group: "Playground",
  entry: "playground",
  import: "CompareGrid",
  barrel: false,
  keyboard: [["Tab", "Reaches each column's thread, which scrolls with the arrow keys"]],
  a11y: [
    "The grid is a named group; each column is a region named after its model, and its thread a region of its own.",
    "On a phone, Previous column and Next column move between columns, and “2 of 3” is announced.",
    "A thread stays at its end while a response grows, unless the person scrolled up to read.",
  ],
};
export const examples = [
  {
    id: "workspace",
    title: "Compare models",
    description: "Send a prompt to all columns: responses stream in, reasoning first. Add Claude Haiku 4.5 to see a rate-limit error and Retry.",
    fn: "Workspace",
    minHeight: 760,
  },
  { id: "two", title: "Two columns", fn: "Two", minHeight: 420 },
];

function Columns({ columns, height = 400 }: { columns: Column[]; height?: number }) {
  return (
    <div className="w-full overflow-hidden rounded-well shadow-ring" style={{ height }}>
      <CompareGrid aria-label="Models" onAdd={() => undefined}>
        {columns.map((c) => {
          const model = playgroundModels.find((m) => m.value === c.model)!;
          const last = c.thread[c.thread.length - 1]?.response;
          return (
            <CompareGrid.Column
              key={c.id}
              label={model.name}
              header={<ModelPicker value={c.model} options={playgroundModels} providers={providers} />}
              onRemove={() => undefined}
              footer={
                <ResponseMetrics
                  timeToFirstToken={last?.metrics.ttft}
                  duration={last?.metrics.duration}
                  inputTokens={last?.metrics.input}
                  outputTokens={last?.metrics.output}
                  reasoningTokens={last?.metrics.reasoningTokens}
                  cost={last?.metrics.cost}
                  live={last?.status === "pending" || last?.status === "streaming"}
                  reasoning={model.reasoning}
                />
              }
            >
              <Thread>
                {c.thread.map((e, i) => (
                  <div key={i} className="flex flex-col gap-12">
                    <UserMessage>{e.prompt}</UserMessage>
                    <ResponseView
                      aria-label={`${model.name} response`}
                      status={e.response.status}
                      text={e.response.text}
                      reasoning={e.response.reasoning}
                      reasoningDuration={e.response.reasoningDuration}
                    />
                  </div>
                ))}
              </Thread>
            </CompareGrid.Column>
          );
        })}
      </CompareGrid>
    </div>
  );
}

export function Workspace() {
  return (
    <div className="h-720 w-full overflow-hidden rounded-well shadow-ring">
      <PlaygroundWorkspace initial={answered(["gpt-5", "claude-sonnet-4-5", "gpt-5-mini"], 1)} />
    </div>
  );
}
export function Two() {
  return <Columns columns={answered(["gpt-5", "claude-sonnet-4-5"], 1)} />;
}
export function States() {
  const streaming = answered(["gpt-5", "claude-sonnet-4-5", "gpt-5-mini"], 1).map((c, i) => {
    const e = c.thread[0]!;
    const r = e.response;
    const response =
      i === 0
        ? { status: "streaming" as const, text: "", reasoning: r.reasoning.slice(0, 210), metrics: { ttft: r.metrics.ttft } }
        : i === 1
          ? { ...r, status: "streaming" as const, text: r.text.slice(0, 190), metrics: { ttft: r.metrics.ttft } }
          : { status: "pending" as const, text: "", reasoning: "", metrics: {} };
    return { ...c, thread: [{ prompt: e.prompt, response }] };
  });
  return (
    <div className="flex flex-col" style={{ width: 1120 }}>
      {[1, 2, 3, 4].map((n) => (
        <StateSection key={n} title={n === 1 ? "One column" : `${n} columns`}>
          <Columns columns={answered(["gpt-5", "claude-sonnet-4-5", "gpt-5-mini", "claude-haiku-4-5"].slice(0, n), 1)} height={360} />
        </StateSection>
      ))}
      <StateSection title="Streaming, thinking first">
        <Columns columns={streaming} height={360} />
      </StateSection>
      <StateSection title="A follow-up">
        <Columns columns={answered(["gpt-5", "claude-sonnet-4-5"], 2)} height={560} />
      </StateSection>
      <StateSection title="Empty columns">
        <Columns columns={answered(["gpt-5", "claude-haiku-4-5"], 0)} height={240} />
      </StateSection>
    </div>
  );
}
