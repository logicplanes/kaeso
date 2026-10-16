import { useEffect, useRef, useState } from "react";
import { Button } from "@logicplanes/kaeso-ui";
import { ResponseMetrics, ResponseView, Thread, UserMessage, type ResponseStatus } from "@logicplanes/kaeso-ui/playground";
import { answers, prompts } from "~/fixtures/playground";
import { StateSection } from "~/lib/states";

const gpt = answers["gpt-5"]![0]!;
const sonnet = answers["claude-sonnet-4-5"]![1]!;

export const meta = {
  title: "ResponseView",
  description: "A model's response as it arrives: Markdown with a quiet caret, reasoning in a Thinking disclosure, and calm stopped and failed states.",
  group: "Playground",
  entry: "playground",
  import: "ResponseView, UserMessage, Thread",
  barrel: false,
  keyboard: [["Enter Space", "Opens and closes Thinking"]],
  a11y: [
    "Each response is an article named by aria-label (“GPT-5 response” in a comparison) and is aria-busy while it streams, so its words aren't read out one by one.",
    "Thinking is a disclosure button; closed, its row shows the latest line of the summary, hidden from screen readers, which read the summary when opened.",
    "A failure says only “No response” in place: the error itself is a toast.",
  ],
};
export const examples = [
  { id: "stream", title: "Streaming", description: "Thinking first, then the answer.", fn: "Stream", minHeight: 560 },
  { id: "thread", title: "A thread", fn: "ThreadDemo", minHeight: 520 },
];

export function Stream() {
  const [state, setState] = useState<{ status: ResponseStatus; text: string; reasoning: string; took?: number; started?: number; total?: number }>({
    status: "done",
    text: gpt.text,
    reasoning: gpt.reasoning!,
    took: 3800,
    total: 6120,
  });
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => () => clearTimeout(timer.current), []);
  const play = () => {
    clearTimeout(timer.current);
    const started = Date.now();
    let r = 0;
    let t = 0;
    let took: number | undefined;
    setState({ status: "pending", text: "", reasoning: "", started });
    const tick = () => {
      if (r < gpt.reasoning!.length) r += 8;
      else {
        took ??= Date.now() - started;
        t += 6;
      }
      const done = t >= gpt.text.length;
      setState({
        status: done ? "done" : "streaming",
        reasoning: gpt.reasoning!.slice(0, r),
        text: gpt.text.slice(0, t),
        started,
        ...(took !== undefined ? { took } : {}),
        ...(done ? { total: Date.now() - started } : {}),
      });
      if (!done) timer.current = setTimeout(tick, 24);
    };
    timer.current = setTimeout(tick, 700);
  };
  return (
    <div className="flex w-full max-w-560 flex-col gap-16">
      <div>
        <Button size="sm" onClick={play} disabled={state.status === "pending" || state.status === "streaming"}>
          Stream it again
        </Button>
      </div>
      <ResponseView
        aria-label="GPT-5 response"
        status={state.status}
        text={state.text}
        reasoning={state.reasoning}
        {...(state.took !== undefined ? { reasoningDuration: state.took } : {})}
        footer={
          <ResponseMetrics
            reasoning
            live={state.status === "pending" || state.status === "streaming"}
            startedAt={state.started}
            duration={state.total}
            timeToFirstToken={state.status === "pending" ? undefined : 1180}
          />
        }
      />
    </div>
  );
}

export function ThreadDemo() {
  return (
    <Thread className="w-full max-w-560">
      <div className="flex flex-col gap-12">
        <UserMessage>{prompts[1]}</UserMessage>
        <ResponseView
          aria-label="Claude Sonnet 4.5 response"
          text={sonnet.text}
          reasoning={sonnet.reasoning}
          reasoningDuration={2100}
          footer={<ResponseMetrics timeToFirstToken={2920} duration={4380} inputTokens={412} outputTokens={168} reasoningTokens={64} cost="0.003756" />}
        />
      </div>
      <div className="flex flex-col gap-12">
        <UserMessage>Shorter subject line, please.</UserMessage>
        <ResponseView aria-label="Claude Sonnet 4.5 response" status="streaming" text={'```json\n{\n  "subject": "Order #4471 is'} />
      </div>
    </Thread>
  );
}

export function States() {
  const half = gpt.text.slice(0, 160);
  return (
    <div className="flex w-560 flex-col">
      <StateSection title="Pending: nothing drawn; the metrics count">
        <ResponseView status="pending" footer={<ResponseMetrics live reasoning />} />
      </StateSection>
      <StateSection title="Thinking, closed: the latest line">
        <ResponseView status="streaming" reasoning={gpt.reasoning!.slice(0, 220)} />
      </StateSection>
      <StateSection title="Thinking, open">
        <ResponseView status="streaming" reasoning={gpt.reasoning!.slice(0, 220)} defaultReasoningOpen />
      </StateSection>
      <StateSection title="Writing">
        <ResponseView status="streaming" reasoning={gpt.reasoning} reasoningDuration={3800} text={half} />
      </StateSection>
      <StateSection title="Done">
        <ResponseView text={gpt.text} reasoning={gpt.reasoning} reasoningDuration={3800} />
      </StateSection>
      <StateSection title="Done, no reasoning">
        <ResponseView text={answers["claude-haiku-4-5"]![0]!.text} />
      </StateSection>
      <StateSection title="Stopped">
        <ResponseView status="stopped" text={half} onRetry={() => undefined} />
      </StateSection>
      <StateSection title="Failed">
        <ResponseView status="error" onRetry={() => undefined} />
      </StateSection>
      <StateSection title="Failed partway">
        <ResponseView status="error" text={half} onRetry={() => undefined} />
      </StateSection>
      <StateSection title="Empty">
        <ResponseView text="" />
      </StateSection>
      <StateSection title="A thread">
        <ThreadDemo />
      </StateSection>
    </div>
  );
}
