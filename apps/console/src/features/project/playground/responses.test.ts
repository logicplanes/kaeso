import { describe, expect, it } from "vitest";
import { applyEvent, endResponse, historyItems, newResponse, type ResponseState, type Turn } from "./responses";

const run = (events: [string, Record<string, unknown>][], start = newResponse(0)) =>
  events.reduce<ResponseState>((s, [type, data], i) => applyEvent(s, type, { type, ...data }, (i + 1) * 10), start);

const reasoningItem = { id: "rs_1", type: "reasoning", summary: [{ type: "summary_text", text: "Plan." }], encrypted_content: "kpp:reasoning:v1:abc" };
const message = { id: "msg_1", type: "message", role: "assistant", content: [{ type: "output_text", text: "Hello there." }] };
const usage = { input_tokens: 12, input_tokens_details: { cached_tokens: 4 }, output_tokens: 30, output_tokens_details: { reasoning_tokens: 9 } };

describe("applyEvent", () => {
  it("builds the text, the reasoning, the usage and the times of a reply", () => {
    const r = run([
      ["response.created", { response: { id: "resp_1" } }],
      ["response.output_item.added", { output_index: 0, item: { id: "rs_1", type: "reasoning" } }],
      ["response.reasoning_summary_part.added", { item_id: "rs_1", summary_index: 0 }],
      ["response.reasoning_summary_text.delta", { item_id: "rs_1", summary_index: 0, delta: "Pl" }],
      ["response.reasoning_summary_text.delta", { item_id: "rs_1", summary_index: 0, delta: "an." }],
      ["response.reasoning_summary_text.delta", { item_id: "rs_1", summary_index: 1, delta: "More." }],
      ["response.output_item.done", { output_index: 0, item: reasoningItem }],
      ["response.output_text.delta", { item_id: "msg_1", output_index: 1, content_index: 0, delta: "Hello " }],
      ["response.output_text.delta", { item_id: "msg_1", output_index: 1, content_index: 0, delta: "there." }],
      ["response.completed", { response: { output: [reasoningItem, message], usage } }],
    ]);
    expect(r.status).toBe("completed");
    expect(r.text).toBe("Hello there.");
    expect(r.reasoning).toBe("Plan.\n\nMore.");
    expect(r.reasoned).toBe(true);
    expect(r.firstTokenAt).toBe(40);
    expect(r.endedAt).toBe(100);
    expect(r.usage).toEqual({ input_tokens: 12, output_tokens: 30, reasoning_tokens: 9, cached_tokens: 4, cache_write_tokens: 0 });
    expect(r.output).toEqual([reasoningItem, message]);
  });

  it("marks a model that reasoned without showing it", () => {
    const r = run([["response.output_item.added", { item: { type: "reasoning" } }]]);
    expect(r).toMatchObject({ status: "streaming", reasoned: true, reasoning: "", firstTokenAt: null });
  });

  it("keeps the gateway's cost when the usage has one", () => {
    expect(run([["response.completed", { response: { usage: { ...usage, cost: "0.00042" } } }]]).usage?.cost).toBe("0.00042");
  });

  it("ends on an error event with its message, and ignores what follows", () => {
    const r = run([
      ["response.output_text.delta", { item_id: "m", delta: "Half" }],
      ["error", { code: "server_is_overloaded", message: "Overloaded" }],
      ["response.output_text.delta", { item_id: "m", delta: " more" }],
    ]);
    expect(r).toMatchObject({ status: "failed", error: "Overloaded", text: "Half", endedAt: 20 });
    expect(run([["error", { error: { message: "Nested." } }]]).error).toBe("Nested.");
    expect(run([["error", {}]]).error).toBe("The model stopped with an error. Try again.");
  });

  it("says why a reply is incomplete, and fails with the response's error", () => {
    expect(run([["response.incomplete", { response: { incomplete_details: { reason: "max_output_tokens" }, usage } }]])).toMatchObject({
      status: "incomplete",
      incompleteReason: "max_output_tokens",
    });
    expect(run([["response.failed", { response: { error: { message: "The provider failed." } } }]])).toMatchObject({
      status: "failed",
      error: "The provider failed.",
    });
  });

  it("names the event from the stream when the data has none, and starts streaming only once", () => {
    const started = applyEvent(newResponse(0), "response.created", {}, 1);
    expect(started.status).toBe("streaming");
    expect(applyEvent(started, "response.in_progress", {}, 2)).toBe(started);
  });

  it("keeps the text of each output apart and ignores empty deltas", () => {
    const r = run([
      ["response.output_item.added", { output_index: 0, item: { type: "message" } }],
      ["response.output_text.delta", { output_index: 0, delta: "First." }],
      ["response.output_text.delta", { output_index: 0, delta: "" }],
      ["response.refusal.delta", { output_index: 1, delta: "I can't." }],
    ]);
    expect(r).toMatchObject({ status: "streaming", reasoned: false, text: "First.\n\nI can't.", firstTokenAt: 20, firstTextAt: 20 });
  });

  it("collects raw reasoning text by its content part", () => {
    const r = run([
      ["response.reasoning_text.delta", { output_index: 0, content_index: 0, delta: "Think" }],
      ["response.reasoning_text.delta", { output_index: 0, content_index: 1, delta: "Again" }],
    ]);
    expect(r).toMatchObject({ reasoning: "Think\n\nAgain", reasoned: true, firstTokenAt: 10 });
  });

  it("keeps finished output items and skips ones without a type", () => {
    const start = run([["response.created", {}]]);
    expect(applyEvent(start, "response.output_item.done", { type: "response.output_item.done" }, 5)).toBe(start);
    expect(applyEvent(start, "response.output_item.done", { type: "response.output_item.done", item: { id: "x" } }, 5)).toBe(start);
    const done = applyEvent(start, "response.output_item.done", { type: "response.output_item.done", output_index: 0, item: message }, 5);
    expect(done).toMatchObject({ output: [message], reasoned: false });
  });

  it("fails with a general message when the failed response says nothing", () => {
    expect(run([["response.failed", {}]]).error).toBe("The model stopped with an error. Try again.");
  });

  it("changes nothing for unknown events", () => {
    const start = newResponse(0);
    expect(applyEvent(start, "response.web_search_call.searching", { type: "response.web_search_call.searching" }, 5)).toBe(start);
  });
});

describe("endResponse", () => {
  it("stops a live reply and keeps its text; a finished one stays as it is", () => {
    const live = run([["response.output_text.delta", { item_id: "m", delta: "Partial" }]]);
    expect(endResponse(live, "stopped", 99)).toMatchObject({ status: "stopped", text: "Partial", endedAt: 99 });
    const done = run([["response.completed", { response: {} }]]);
    expect(endResponse(done, "stopped", 99)).toBe(done);
  });
});

describe("historyItems", () => {
  const turn = (prompt: string, response: ResponseState): Turn => ({ id: prompt, prompt, response });
  const completed = run([["response.completed", { response: { output: [reasoningItem, message], usage } }]]);

  it("sends finished turns back with their reasoning items as received and the reply as an assistant message", () => {
    expect(historyItems([turn("Hi", completed)])).toEqual([{ role: "user", content: "Hi" }, reasoningItem, { role: "assistant", content: "Hello there." }]);
  });

  it("keeps the text of a stopped turn, leaves out failed ones without text and live ones", () => {
    const stopped = endResponse(run([["response.output_text.delta", { item_id: "m", delta: "Partial" }]]), "stopped", 1);
    const failed = endResponse(newResponse(0), "failed", 1, "Budget used up.");
    expect(historyItems([turn("A", stopped), turn("B", failed), turn("C", newResponse(0))])).toEqual([
      { role: "user", content: "A" },
      { role: "assistant", content: "Partial" },
    ]);
  });

  it("sends refusals back and leaves out empty messages and other items", () => {
    const mixed = run([
      [
        "response.completed",
        {
          response: {
            output: [
              { type: "message", content: [{ type: "refusal", refusal: "I can't help with that." }, { type: "annotation" }] },
              { type: "message" },
              { type: "function_call", name: "lookup" },
            ],
          },
        },
      ],
    ]);
    expect(historyItems([turn("Hi", mixed)])).toEqual([
      { role: "user", content: "Hi" },
      { role: "assistant", content: "I can't help with that." },
    ]);
  });

  it("sends only the prompt of a finished turn that returned nothing", () => {
    expect(historyItems([turn("Hi", run([["response.completed", { response: {} }]]))])).toEqual([{ role: "user", content: "Hi" }]);
  });

  it("leaves out reasoning without encrypted content", () => {
    const plain = run([["response.completed", { response: { output: [{ type: "reasoning", summary: [] }, message] } }]]);
    expect(historyItems([turn("Hi", plain)])).toEqual([
      { role: "user", content: "Hi" },
      { role: "assistant", content: "Hello there." },
    ]);
  });
});
