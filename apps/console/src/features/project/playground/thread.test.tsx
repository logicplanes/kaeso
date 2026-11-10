import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import type { UserMessageProps } from "@logicplanes/kaeso-ui/playground";
import { playgroundModel } from "~/test/fixtures/project-logs";
import { applyEvent, newResponse, type ResponseState, type Turn } from "./responses";
import Thread from "./thread";

const rendered = vi.hoisted(() => new Map<string, number>());
vi.mock("@logicplanes/kaeso-ui/playground", async (original) => {
  const ui = await original<typeof import("@logicplanes/kaeso-ui/playground")>();
  function CountedMessage(props: UserMessageProps) {
    const key = String(props.children);
    rendered.set(key, (rendered.get(key) ?? 0) + 1);
    return <ui.UserMessage {...props} />;
  }
  return { ...ui, UserMessage: CountedMessage };
});

const delta = (r: ResponseState, text: string, at: number) =>
  applyEvent(r, "response.output_text.delta", { type: "response.output_text.delta", item_id: "m", delta: text }, at);
const finished = (prompt: string, text: string): Turn => ({
  id: prompt,
  prompt,
  response: applyEvent(delta(newResponse(0), text, 1), "response.completed", { type: "response.completed", response: {} }, 2),
});

describe("Thread", () => {
  it("renders only the turn that streams on each frame of a long reply, never the finished ones", () => {
    rendered.clear();
    const model = playgroundModel();
    const earlier = [finished("First question", "First answer."), finished("Second question", "Second answer.")];
    let live: Turn = { id: "third", prompt: "Third question", response: newResponse(0) };
    const view = (turns: Turn[]) => <Thread turns={turns} model={model} label="GPT-5.4 mini" threadKey="col1" onRetry={null} />;
    const { rerender } = render(view([...earlier, live]));
    for (let frame = 1; frame <= 120; frame++) {
      live = { ...live, response: delta(live.response, `Line ${frame} of a long reply.\n\n`, frame) };
      rerender(view([...earlier, live]));
    }
    expect(rendered.get("First question")).toBe(1);
    expect(rendered.get("Second question")).toBe(1);
    expect(rendered.get("Third question")).toBe(121);
    expect(screen.getByText("Line 120 of a long reply.")).toBeInTheDocument();
  });

  it("shows Retry under the last turn only", () => {
    render(
      <Thread
        turns={[finished("One", "Answer one."), finished("Two", "Answer two.")]}
        model={null}
        label="GPT-5.4 mini"
        threadKey="col1"
        onRetry={() => undefined}
      />,
    );
    expect(screen.getAllByRole("button", { name: "Retry" })).toHaveLength(1);
  });
});
