import { useEffect, useRef, useState } from "react";
import { Button } from "@logicplanes/kaeso-ui";
import { ResponseMetrics, ResponseView, Thread, UserMessage, type ResponseStatus } from "@logicplanes/kaeso-ui/playground";
import { answers, prompts } from "~/fixtures/playground";
import { StateSection } from "~/lib/states";

const gpt = answers["gpt-5"]![0]!;
const sonnet = answers["claude-sonnet-4-5"]![1]!;

export const meta = {
  title: "ResponseView",
  description:
    "A model's response as it arrives: a quiet caret until the first token, Markdown as it streams, reasoning in a Thinking disclosure, and calm stopped, cut-off and failed states.",
  group: "Playground",
  entry: "playground",
  import: "ResponseView, UserMessage, Thread",
  barrel: false,
  keyboard: [["Enter Space", "Opens and closes Thinking"]],
  a11y: [
    "Each response is an article named by aria-label (“GPT-5 response” in a comparison) and is aria-busy while it streams, so its words aren't read out one by one.",
    "Thinking is a disclosure button; closed, its row shows the latest line of the summary, hidden from screen readers, which read the summary when opened.",
    "A failure is an alert in place of the response: “No response”, then the error.",
    "When a response finishes, stops or is cut off, a status message says so once, by name: “GPT-5 response finished”.",
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
  const table =
    "Two did:\n\n| Model | Errors | Rate |\n|---|--:|--:|\n| gpt-5 | 41 | 0.9% |\n| claude-haiku-4-5 | 12 | 0.3% |\n\nMost were **429s** from the project's limit.";
  return (
    <div className="flex w-560 flex-col">
      <StateSection title="Pending: a quiet mark until the first token">
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
      <StateSection title="Writing, reasoning open">
        <ResponseView status="streaming" reasoning={gpt.reasoning} reasoningDuration={3800} text={half} defaultReasoningOpen />
      </StateSection>
      <StateSection title="Done">
        <ResponseView text={gpt.text} reasoning={gpt.reasoning} reasoningDuration={3800} />
      </StateSection>
      <StateSection title="Done, no reasoning, with metrics">
        <ResponseView
          text={answers["claude-haiku-4-5"]![0]!.text}
          footer={<ResponseMetrics timeToFirstToken={280} duration={1460} inputTokens={98} outputTokens={41} cost="0.000026" />}
        />
      </StateSection>
      <StateSection title="A table and code">
        <ResponseView text={`${table}\n\n\`\`\`json\n{ "model": "gpt-5", "errors": 41 }\n\`\`\``} />
      </StateSection>
      <StateSection title="Stopped">
        <ResponseView status="stopped" text={half} onRetry={() => undefined} />
      </StateSection>
      <StateSection title="Stopped before a response">
        <ResponseView status="stopped" onRetry={() => undefined} />
      </StateSection>
      <StateSection title="Cut off at the output limit">
        <ResponseView status="incomplete" incompleteReason="max_output_tokens" text={half} onRetry={() => undefined} />
      </StateSection>
      <StateSection title="Cut off by the content filter">
        <ResponseView status="incomplete" incompleteReason="content_filter" text={half} onRetry={() => undefined} />
      </StateSection>
      <StateSection title="Ended early, no reason given">
        <ResponseView status="incomplete" text={half} />
      </StateSection>
      <StateSection title="Failed">
        <ResponseView status="error" error="Budget used up: the project's $5.00 per month." onRetry={() => undefined} />
      </StateSection>
      <StateSection title="Failed partway">
        <ResponseView status="error" text={half} error="The stream ended before the model finished. Try again." onRetry={() => undefined} />
      </StateSection>
      <StateSection title="Failed, no Retry">
        <ResponseView status="error" error="The key `sk-kso-…wxyz` can't use GPT-5. Ask an owner to allow it." />
      </StateSection>
      <StateSection title="Empty">
        <ResponseView text="" />
      </StateSection>
      <StateSection title="A long prompt, clamped">
        <UserMessage>{Array.from({ length: 3 }, () => prompts[0]).join(" ")}</UserMessage>
      </StateSection>
      <StateSection title="A thread">
        <ThreadDemo />
      </StateSection>
    </div>
  );
}
