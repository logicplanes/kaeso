import { CompareGrid, ModelPicker, ResponseMetrics, ResponseView, Thread, UserMessage } from "@logicplanes/kaeso-ui/playground";
import { playgroundModels } from "~/fixtures/playground";
import { providers } from "~/fixtures/kaeso";
import { EmptyColumn, PlaygroundWorkspace, answered, type Column } from "./_playground-workspace";
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
    "On a phone, or when the columns don't fit, Previous column and Next column move between columns, and “Claude Sonnet 4.5, 2 of 3” is announced.",
    "Adding a column moves the focus to its header; removing one moves it to the column that takes its place.",
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

function Columns({ columns, height = 400, divider }: { columns: Column[]; height?: number; divider?: "hairline" | "none" }) {
  return (
    <div className="w-full overflow-hidden rounded-well shadow-ring" style={{ height }}>
      <CompareGrid aria-label="Models" onAdd={() => undefined} {...(divider ? { divider } : {})}>
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
              {c.thread.length === 0 ? (
                <EmptyColumn name={model.name} />
              ) : (
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
                        error={e.response.error}
                        {...(e.response.status === "error" ? { onRetry: () => undefined } : {})}
                      />
                    </div>
                  ))}
                </Thread>
              )}
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
  const failed = answered(["gpt-5", "claude-haiku-4-5"], 1).map((c, i) =>
    i === 1
      ? {
          ...c,
          thread: [
            {
              prompt: c.thread[0]!.prompt,
              response: {
                status: "error" as const,
                text: "",
                reasoning: "",
                error: "The project allows 60 requests a minute to Claude Haiku 4.5. Try again in 20 seconds.",
                metrics: {},
              },
            },
          ],
        }
      : c,
  );
  const four = ["gpt-5", "claude-sonnet-4-5", "gpt-5-mini", "claude-haiku-4-5"];
  return (
    <div className="flex w-640 flex-col">
      <StateSection title="One column">
        <Columns columns={answered(four.slice(0, 1), 1)} height={360} />
      </StateSection>
      <StateSection title="More columns than fit: Previous and Next, faded edges">
        <Columns columns={answered(four.slice(0, 2), 1)} height={400} />
      </StateSection>
      <StateSection title="4 columns: Add is off at the maximum, with its reason">
        <Columns columns={answered(four, 1)} height={400} />
      </StateSection>
      <StateSection title="Streaming, thinking first">
        <Columns columns={streaming} height={360} />
      </StateSection>
      <StateSection title="A column that failed">
        <Columns columns={failed} height={360} />
      </StateSection>
      <StateSection title="A follow-up">
        <Columns columns={answered(["gpt-5", "claude-sonnet-4-5"], 2)} height={560} />
      </StateSection>
      <StateSection title="Empty columns">
        <Columns columns={answered(["gpt-5", "claude-haiku-4-5"], 0)} height={240} />
      </StateSection>
      <StateSection title="No divider">
        <Columns columns={answered(["gpt-5", "claude-sonnet-4-5"], 1)} height={320} divider="none" />
      </StateSection>
    </div>
  );
}
