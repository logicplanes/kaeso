import { useCallback, useEffect, useRef, useState } from "react";
import { Button, icons } from "@logicplanes/kaeso-ui";
import {
  CompareGrid,
  Composer,
  ModelPicker,
  ParameterPopover,
  PromptSuggestions,
  ResponseMetrics,
  ResponseView,
  Thread,
  UserMessage,
  type PlaygroundParameters,
  type ResponseStatus,
} from "@logicplanes/kaeso-ui/playground";
import { answers, costOf, instructions, playgroundModels, prompts, tokensOf, type PlaygroundModel } from "~/fixtures/playground";

export interface Metrics {
  ttft?: number | undefined;
  duration?: number | undefined;
  input?: number | undefined;
  output?: number | undefined;
  reasoningTokens?: number | undefined;
  cost?: string | undefined;
}
export interface Response {
  status: ResponseStatus;
  text: string;
  reasoning: string;
  reasoningDuration?: number | undefined;
  startedAt?: number | undefined;
  error?: string | undefined;
  metrics: Metrics;
}
export interface Entry {
  prompt: string;
  response: Response;
}
export interface Column {
  id: string;
  model: string;
  thread: Entry[];
}

const modelOf = (value: string) => playgroundModels.find((m) => m.value === value) ?? playgroundModels[0]!;
const answerOf = (model: string, turn: number) => {
  const list = answers[model] ?? answers["gpt-5"]!;
  return list[turn] ?? list[0]!;
};

export function finished(model: PlaygroundModel, turn: number, history: number): Response {
  const a = answerOf(model.value, turn);
  const output = tokensOf(a.text);
  const reasoningTokens = a.reasoning ? tokensOf(a.reasoning) * 4 : undefined;
  const input = 64 + history;
  const reasoningDuration = a.reasoning ? Math.round(a.reasoning.length * 9 + 700) : undefined;
  return {
    status: "done",
    text: a.text,
    reasoning: a.reasoning ?? "",
    reasoningDuration,
    metrics: {
      ttft: model.latency + (reasoningDuration ?? 0),
      duration: model.latency + (reasoningDuration ?? 0) + Math.round((a.text.length / model.speed) * 20),
      input,
      output: output + (reasoningTokens ?? 0),
      reasoningTokens,
      cost: costOf(model, input, output + (reasoningTokens ?? 0)),
    },
  };
}

export function answered(models: string[], turns = prompts.length): Column[] {
  return models.map((model, i) => {
    const thread: Entry[] = [];
    let history = 0;
    for (let t = 0; t < turns; t++) {
      const response = finished(modelOf(model), t, history);
      thread.push({ prompt: prompts[t] ?? prompts[0]!, response });
      history += tokensOf(prompts[t] ?? "") + (response.metrics.output ?? 0);
    }
    return { id: `col_${i + 1}`, model, thread };
  });
}

function useFakeGateway(setColumns: (f: (c: Column[]) => Column[]) => void) {
  const timers = useRef(new Map<string, ReturnType<typeof setTimeout>>());
  const limited = useRef(new Set<string>());
  useEffect(() => {
    const t = timers.current;
    return () => {
      for (const id of t.values()) clearTimeout(id);
    };
  }, []);

  const patch = useCallback(
    (col: string, f: (r: Response) => Response) =>
      setColumns((cs) =>
        cs.map((c) => {
          if (c.id !== col) return c;
          const last = c.thread[c.thread.length - 1];
          if (!last) return c;
          return { ...c, thread: [...c.thread.slice(0, -1), { ...last, response: f(last.response) }] };
        }),
      ),
    [setColumns],
  );

  const run = useCallback(
    (col: Column, turn: number) => {
      const model = modelOf(col.model);
      const a = answerOf(model.value, turn);
      const started = performance.now();
      const history = col.thread.slice(0, -1).reduce((n, e) => n + tokensOf(e.prompt) + (e.response.metrics.output ?? 0), 0);
      const input = 64 + history + tokensOf(col.thread[col.thread.length - 1]?.prompt ?? "");
      const step = (fn: () => void, ms: number) => timers.current.set(col.id, setTimeout(fn, ms));
      if (model.value === "claude-haiku-4-5" && !limited.current.has(col.id)) {
        limited.current.add(col.id);
        step(() => {
          patch(col.id, (r) => ({ ...r, status: "error", error: "The project allows 60 requests a minute to Claude Haiku 4.5. Try again in 20 seconds." }));
        }, 420);
        return;
      }
      let ttft: number | undefined;
      let reasoned = 0;
      let written = 0;
      let reasoningDuration: number | undefined;
      const tick = () => {
        const now = performance.now() - started;
        if (a.reasoning && reasoned < a.reasoning.length) {
          ttft ??= Math.round(now);
          reasoned = Math.min(a.reasoning.length, reasoned + model.speed + 3);
          patch(col.id, (r) => ({ ...r, status: "streaming", reasoning: a.reasoning!.slice(0, reasoned), metrics: { ...r.metrics, ttft } }));
          step(tick, 20);
          return;
        }
        if (a.reasoning && reasoningDuration === undefined) reasoningDuration = Math.round(now);
        if (written < a.text.length) {
          ttft ??= Math.round(now);
          written = Math.min(a.text.length, written + model.speed);
          patch(col.id, (r) => ({ ...r, status: "streaming", text: a.text.slice(0, written), reasoningDuration, metrics: { ...r.metrics, ttft } }));
          step(tick, 20);
          return;
        }
        const reasoningTokens = a.reasoning ? tokensOf(a.reasoning) * 4 : undefined;
        const output = tokensOf(a.text) + (reasoningTokens ?? 0);
        patch(col.id, (r) => ({
          ...r,
          status: "done",
          metrics: { ttft, duration: Math.round(performance.now() - started), input, output, reasoningTokens, cost: costOf(model, input, output) },
        }));
        timers.current.delete(col.id);
      };
      step(tick, model.latency);
    },
    [patch],
  );

  const stop = useCallback(() => {
    for (const [col, id] of timers.current) {
      clearTimeout(id);
      patch(col, (r) => (r.status === "pending" || r.status === "streaming" ? { ...r, status: "stopped" } : r));
    }
    timers.current.clear();
  }, [patch]);

  return { run, stop };
}

const suggestions = ["Apologize for a late order in two sentences", "Explain our refund policy in plain words", "Sort these support emails by topic"];

const empty = (): Response => ({ status: "pending", text: "", reasoning: "", startedAt: Date.now(), metrics: {} });

export function PlaygroundWorkspace({ initial, initialPrompt = "" }: { initial: Column[]; initialPrompt?: string }) {
  const [columns, setColumns] = useState(initial);
  const [params, setParams] = useState<PlaygroundParameters>({ instructions });
  const [draft, setDraft] = useState(initialPrompt);
  const gateway = useFakeGateway(setColumns);
  const streaming = columns.some((c) => c.thread.some((e) => e.response.status === "pending" || e.response.status === "streaming"));
  const next = useRef(initial.length + 1);
  const prompt = useRef<HTMLTextAreaElement | null>(null);

  const send = (prompt: string) => {
    const sent = columns.map((c) => ({ ...c, thread: [...c.thread, { prompt, response: empty() }] }));
    setColumns(sent);
    setDraft("");
    for (const c of sent) gateway.run(c, c.thread.length - 1);
  };
  const retry = (id: string) => {
    const c = columns.find((x) => x.id === id);
    const last = c?.thread[c.thread.length - 1];
    if (!c || !last) return;
    const again = { ...c, thread: [...c.thread.slice(0, -1), { ...last, response: empty() }] };
    setColumns((cs) => cs.map((x) => (x.id === id ? again : x)));
    gateway.run(again, again.thread.length - 1);
  };
  const add = () => {
    const used = new Set(columns.map((c) => c.model));
    const model = playgroundModels.find((m) => !used.has(m.value))?.value ?? "gpt-5";
    setColumns((cs) => [...cs, { id: `col_${next.current++}`, model, thread: [] }]);
  };
  const reasons = columns.some((c) => modelOf(c.model).reasoning);
  const fresh = columns.every((c) => c.thread.length === 0);

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="min-h-0 flex-1">
        <CompareGrid aria-label="Models" onAdd={add}>
          {columns.map((c) => {
            const model = modelOf(c.model);
            const last = c.thread[c.thread.length - 1]?.response;
            return (
              <CompareGrid.Column
                key={c.id}
                label={model.name}
                header={
                  <ModelPicker
                    value={c.model}
                    options={playgroundModels}
                    onValueChange={(v) => setColumns((cs) => cs.map((x) => (x.id === c.id ? { ...x, model: v } : x)))}
                  />
                }
                onRemove={() => setColumns((cs) => cs.filter((x) => x.id !== c.id))}
                footer={
                  <ResponseMetrics
                    timeToFirstToken={last?.metrics.ttft}
                    duration={last?.metrics.duration}
                    inputTokens={last?.metrics.input}
                    outputTokens={last?.metrics.output}
                    reasoningTokens={last?.metrics.reasoningTokens}
                    reasoning={model.reasoning}
                    live={last?.status === "pending" || last?.status === "streaming"}
                    startedAt={last?.startedAt}
                    cost={last?.metrics.cost}
                  />
                }
              >
                {c.thread.length === 0 ? (
                  <div className="flex flex-1 flex-col items-center justify-center gap-4 px-16 text-center">
                    <p className="text-body font-medium text-fg">{model.name}</p>
                    <p className="max-w-280 text-small text-fg-secondary text-pretty">
                      Its answers appear here, side by side with the others, from the next prompt you send.
                    </p>
                  </div>
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
                          {...(i === c.thread.length - 1 ? { onRetry: () => retry(c.id) } : {})}
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
      <div className="flex shrink-0 flex-col gap-12 px-12 pt-4 pb-12 md:px-16 md:pb-16">
        {fresh && (
          <PromptSuggestions
            className="mx-auto w-full max-w-800"
            suggestions={suggestions}
            onSelect={(p) => {
              setDraft(p);
              prompt.current?.focus();
            }}
          />
        )}
        <Composer
          ref={prompt}
          className="mx-auto w-full max-w-800"
          aria-label="Message all models"
          placeholder={columns.length > 1 ? `Message ${columns.length} models` : "Message the model"}
          value={draft}
          onValueChange={setDraft}
          onSubmit={send}
          onStop={gateway.stop}
          streaming={streaming}
          actions={
            <ParameterPopover
              value={params}
              onValueChange={setParams}
              reasoning={reasons}
              side="top"
              align="start"
              trigger={
                <Button size="sm" variant="ghost" leadingIcon={icons.displayOptions}>
                  Parameters
                </Button>
              }
            />
          }
        />
      </div>
    </div>
  );
}
