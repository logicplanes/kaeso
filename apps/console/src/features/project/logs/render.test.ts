import { describe, expect, it } from "vitest";
import { embeddingsView, isKnownReplyForm, replyView, requestView, stopLabel } from "./render";

const anthropicStream = {
  request: { max_tokens: 300, messages: [{ content: "Stream a short answer.", role: "user" }], model: "fast", stream: true },
  reply: {
    blocks: [{ text: { text: "Hello! It's nice to meet you. How can I help you today?" } }],
    id: "msg_01YPYNJ43PX3m1nxqjPEjfaw",
    model: "claude-opus-5-5",
    stop: "end_turn",
    usage: { cache_read_tokens: 0, cache_write_tokens: 0, input_tokens: 10, output_tokens: 18 },
  },
};
const responsesTools = {
  request: { input: "Look up the order. [mock:fixture=recorded-tools-function]", model: "gpt-6-sol" },
  reply: {
    blocks: [{ tool_call: { arguments: { location: "Boston, MA", unit: "fahrenheit" }, id: "call_wdEvTxRQoNKByBADWmlV9V9W", name: "get_current_weather" } }],
    id: "resp_1",
    model: "gpt-6-sol",
    stop: "tool_use",
    usage: { input_tokens: 10, output_tokens: 5 },
  },
};

describe("requestView", () => {
  it("maps Anthropic Messages with system, tools and parameters", () => {
    const v = requestView("anthropic_messages", {
      ...anthropicStream.request,
      system: "Be brief.",
      tools: [{ name: "lookup", description: "Find an order", input_schema: { type: "object" } }],
      messages: [
        {
          role: "user",
          content: [
            { type: "text", text: "Where is order 7?" },
            { type: "image", source: { type: "base64", media_type: "image/png", data: "iVBOR" } },
          ],
        },
        {
          role: "assistant",
          content: [
            { type: "thinking", thinking: "Look it up." },
            { type: "tool_use", id: "tu_1", name: "lookup", input: { order: 7 } },
          ],
        },
        { role: "user", content: [{ type: "tool_result", tool_use_id: "tu_1", content: "Shipped" }] },
      ],
    });
    expect(v?.messages.map((m) => m.role)).toEqual(["system", "user", "assistant", "user"]);
    expect(v?.messages[1]?.content).toContainEqual({ type: "image", src: "data:image/png;base64,iVBOR", media_type: "image/png" });
    expect(v?.messages[2]?.content).toContainEqual({ type: "tool_call", id: "tu_1", name: "lookup", arguments: { order: 7 } });
    expect(v?.tools).toEqual([{ name: "lookup", description: "Find an order", schema: { type: "object" } }]);
    expect(v?.params).toContainEqual({ label: "Max tokens", value: "300" });
    expect(v?.params).toContainEqual({ label: "Streamed", value: "true" });
  });
  it("maps OpenAI Chat tool calls with their JSON arguments, and tool results", () => {
    const v = requestView("openai_chat", {
      messages: [
        { role: "user", content: "Weather?" },
        { role: "assistant", content: null, tool_calls: [{ id: "c1", type: "function", function: { name: "weather", arguments: '{"city":"Oslo"}' } }] },
        { role: "tool", tool_call_id: "c1", content: "4°C" },
      ],
    });
    expect(v?.messages[1]?.content).toEqual([{ type: "tool_call", id: "c1", name: "weather", arguments: { city: "Oslo" } }]);
    expect(v?.messages[2]).toEqual({ role: "tool", content: [{ type: "tool_result", tool_call_id: "c1", content: "4°C" }] });
  });
  it("maps an OpenAI Responses string input", () => {
    expect(requestView("openai_responses", responsesTools.request)?.messages).toEqual([{ role: "user", content: responsesTools.request.input }]);
  });
  it("returns null for anything it can't map, instead of guessing", () => {
    expect(requestView("anthropic_messages", { messages: [{ role: "user", content: [{ type: "server_tool_use" }] }] })).toBeNull();
    expect(requestView("openai_chat", { messages: [{ role: "robot", content: "x" }] })).toBeNull();
    expect(requestView("bedrock", {})).toBeNull();
  });
  it("counts embedding inputs", () => {
    expect(embeddingsView("openai_embeddings", { input: ["refund policy", "shipping times"] })).toEqual({ inputs: 2 });
    expect(embeddingsView("openai_embeddings", { input: "one" })).toEqual({ inputs: 1 });
  });
});

const blob = (block: object) => "kpp:reasoning:v1:" + btoa(JSON.stringify(block)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
const anthropic = { dialect: "anthropic_messages", provider: "anthropic" };

describe("reasoning", () => {
  const turn2 = {
    model: "claude-haiku-5-5",
    store: false,
    include: ["reasoning.encrypted_content"],
    reasoning: { effort: "high", summary: "auto" },
    input: [
      { role: "user", content: "Check the weather in Oslo with the tool first." },
      {
        id: "rs_kaeso_msg01Xq_0",
        type: "reasoning",
        summary: [{ type: "summary_text", text: "I'll check the weather in Oslo first." }],
        encrypted_content: blob({
          origin: anthropic,
          text: ["I'll check the weather in Oslo first."],
          opaque: [{ origin: anthropic, kind: "anthropic_thinking_signature", data: "c2lnbmF0dXJl" }],
        }),
      },
      { id: "rs_open", type: "reasoning", summary: [], content: [{ type: "reasoning_text", text: "Boiling point at sea level." }] },
      { id: "rs_hidden", type: "reasoning", summary: [], encrypted_content: "gAAAAABplaceholder" },
      {
        id: "rs_kaeso_msg01Xq_1",
        type: "reasoning",
        summary: [],
        encrypted_content: blob({ origin: anthropic, opaque: [{ origin: anthropic, kind: "anthropic_redacted_thinking", data: "cmVkYWN0ZWQ" }] }),
      },
      { type: "function_call", call_id: "toolu_01", name: "get_weather", arguments: '{"city":"Oslo"}' },
      { type: "function_call_output", call_id: "toolu_01", output: '{"temp_c": 3}' },
    ],
  };

  it("shows a Responses reasoning item's summary and reasoning text, and says why one has none", () => {
    const v = requestView("openai_responses", turn2);
    const shown = v?.messages.flatMap((m) => (typeof m.content === "string" ? [] : m.content)).filter((b) => b.type === "reasoning");
    expect(shown).toEqual([
      { type: "reasoning", text: "I'll check the weather in Oslo first." },
      { type: "reasoning", text: "Boiling point at sea level." },
      { type: "reasoning", text: "" },
      { type: "reasoning", text: "", redacted: true },
    ]);
    const json = JSON.stringify(v);
    expect(json).not.toContain("kpp:reasoning");
    expect(json).not.toContain("gAAAA");
    expect(json).not.toContain("c2lnbmF0dXJl");
  });

  it("tells hidden thinking from redacted thinking in Anthropic requests and KPP replies", () => {
    const v = requestView("anthropic_messages", {
      model: "claude-opus-5-5",
      max_tokens: 1000,
      messages: [
        { role: "user", content: "Hi" },
        {
          role: "assistant",
          content: [
            { type: "thinking", thinking: "", signature: "EqQBCkYIBhgC" },
            { type: "redacted_thinking", data: "EmwKAhgBEgy" },
            { type: "text", text: "Hello." },
          ],
        },
      ],
    });
    expect(v?.messages[1]?.content).toEqual([
      { type: "reasoning", text: "" },
      { type: "reasoning", text: "", redacted: true },
      { type: "text", text: "Hello." },
    ]);
    const r = replyView({
      blocks: [
        { reasoning: { origin: anthropic, text: [""], opaque: [{ origin: anthropic, kind: "anthropic_thinking_signature", data: "EqQB" }] } },
        { reasoning: { origin: anthropic, opaque: [{ origin: anthropic, kind: "anthropic_redacted_thinking", data: "EmwK" }] } },
      ],
      stop: "end_turn",
      model: "m",
      id: "r",
    });
    expect(r?.blocks).toEqual([
      { type: "reasoning", text: "" },
      { type: "reasoning", text: "", redacted: true },
    ]);
  });
});

describe("replyView", () => {
  it("reads KPP's externally tagged blocks and stop reason", () => {
    const v = replyView(anthropicStream.reply);
    expect(v?.blocks).toEqual([{ type: "text", text: "Hello! It's nice to meet you. How can I help you today?" }]);
    expect(v?.stop).toBe("End of turn");
    expect(replyView(responsesTools.reply)?.blocks[0]).toMatchObject({ type: "tool_call", name: "get_current_weather" });
  });
  it("shows reasoning text and says how much provider-signed data it can't read", () => {
    const v = replyView({
      blocks: [{ reasoning: { summary: ["Checked the order."], opaque: [{ origin: {}, kind: "signature", data: "abcd" }] } }],
      stop: "end_turn",
      model: "m",
      id: "r",
    });
    expect(v?.blocks[0]).toEqual({ type: "reasoning", text: "Checked the order.\n\n4 bytes of provider-signed data, not readable." });
  });
  it("keeps unknown blocks aside as JSON and names other stop reasons", () => {
    const v = replyView({ blocks: [{ unknown: { type: "web_search_tool_result" } }], stop: { stop_sequence: "END" }, model: "m", id: "r" });
    expect(v?.blocks).toEqual([]);
    expect(v?.unknown).toEqual([{ type: "web_search_tool_result" }]);
    expect(v?.stop).toBe('Stop sequence "END"');
    expect(stopLabel("max_tokens")).toBe("Max tokens reached");
  });
  it("is null for a missing reply", () => {
    expect(replyView(null)).toBeNull();
  });
  it("reads the form without a format field and refuses every form with one", () => {
    const blocks = [{ text: { text: "Hi" } }];
    expect(isKnownReplyForm({ blocks, stop: "end_turn", model: "m", id: "r" })).toBe(true);
    expect(isKnownReplyForm(null)).toBe(true);
    for (const format of [2, 1, "2", null]) {
      const reply = { format, blocks, stop: "end_turn", model: "m", id: "r" };
      expect(isKnownReplyForm(reply)).toBe(false);
      expect(replyView(reply)).toBeNull();
    }
  });
});

describe("requestView, every part", () => {
  it("maps Anthropic image and document sources, nested tool results and hosted tools", () => {
    const v = requestView("anthropic_messages", {
      system: [{ type: "text", text: "Be kind." }],
      tools: [{ type: "web_search_20250305" }],
      messages: [
        {
          role: "user",
          content: [
            { type: "image", source: { type: "url", url: "https://example.com/cat.png", media_type: "image/png" } },
            { type: "image", source: { type: "url", url: "https://example.com/dog.png" } },
            { type: "image", source: { type: "file", media_type: "image/webp" } },
            { type: "image", source: { type: "file" } },
            { type: "image", source: { type: "base64", data: "AAAA" } },
            { type: "image" },
            { type: "document", title: "Terms", source: { type: "base64", media_type: "application/pdf", data: "JVBE" } },
            { type: "document", source: { type: "text" } },
          ],
        },
        {
          role: "user",
          content: [
            { type: "tool_result", tool_use_id: "tu_1", content: [{ type: "text", text: "Found it" }], is_error: true },
            { type: "tool_result", content: undefined },
          ],
        },
        { role: "assistant", content: [{ type: "tool_use", name: "lookup", input: {} }] },
      ],
    });
    expect(v?.messages[0]).toEqual({ role: "system", content: [{ type: "text", text: "Be kind." }] });
    expect(v?.messages[1]?.content).toEqual([
      { type: "image", src: "https://example.com/cat.png", media_type: "image/png" },
      { type: "image", src: "https://example.com/dog.png" },
      { type: "image", media_type: "image/webp" },
      { type: "image" },
      { type: "image", src: "data:application/octet-stream;base64,AAAA" },
      { type: "image" },
      { type: "document", name: "Terms", media_type: "application/pdf" },
      { type: "document" },
    ]);
    expect(v?.messages[2]?.content).toEqual([
      { type: "tool_result", tool_call_id: "tu_1", content: [{ type: "text", text: "Found it" }], is_error: true },
      { type: "tool_result", content: [] },
    ]);
    expect(v?.messages[3]?.content).toEqual([{ type: "tool_call", name: "lookup", arguments: {} }]);
    expect(v?.tools).toEqual([{ name: "web_search_20250305" }]);
  });

  it("refuses Anthropic shapes it doesn't know", () => {
    expect(requestView("anthropic_messages", { messages: ["hi"] })).toBeNull();
    expect(requestView("anthropic_messages", { messages: [{ role: "user", content: ["hi"] }] })).toBeNull();
    expect(requestView("anthropic_messages", { messages: [{ role: "user", content: [{ type: "text" }] }] })).toBeNull();
    expect(requestView("anthropic_messages", { messages: [], tools: ["lookup"] })).toBeNull();
    expect(requestView("anthropic_messages", { messages: [], tools: [{ description: "no name" }] })).toBeNull();
    expect(requestView("anthropic_messages", "not an object")).toBeNull();
  });

  it("maps OpenAI Chat images, files, refusals and tool calls", () => {
    const v = requestView("openai_chat", {
      messages: [
        {
          role: "user",
          content: [
            { type: "text", text: "Look" },
            { type: "image_url", image_url: { url: "https://example.com/a.png" } },
            { type: "image_url" },
            { type: "file", file: { filename: "report.pdf" } },
            { type: "file" },
          ],
        },
        { role: "assistant", content: [{ type: "refusal", refusal: "No." }, { type: "refusal" }], refusal: "I can't help with that." },
        { role: "assistant", tool_calls: [{ function: { name: "draw", arguments: "not json" } }] },
        { role: "tool", content: [{ type: "text", text: "drawn" }] },
      ],
      tools: [{ type: "function", function: { name: "draw" } }, { type: "code_interpreter" }],
      stop: ["END"],
      seed: 7,
      metadata: null,
    });
    expect(v?.messages[0]?.content).toEqual([
      { type: "text", text: "Look" },
      { type: "image", src: "https://example.com/a.png" },
      { type: "image" },
      { type: "document", name: "report.pdf" },
      { type: "document" },
    ]);
    expect(v?.messages[1]?.content).toEqual([
      { type: "refusal", text: "No." },
      { type: "refusal", text: "" },
      { type: "refusal", text: "I can't help with that." },
    ]);
    expect(v?.messages[2]?.content).toEqual([{ type: "tool_call", name: "draw", arguments: "not json" }]);
    expect(v?.messages[3]).toEqual({ role: "tool", content: [{ type: "tool_result", content: [{ type: "text", text: "drawn" }] }] });
    expect(v?.tools).toEqual([{ name: "draw" }, { name: "code_interpreter" }]);
    expect(v?.params).toEqual([
      { label: "Stop", value: '["END"]' },
      { label: "Seed", value: "7" },
    ]);
  });

  it("refuses OpenAI Chat shapes it doesn't know", () => {
    expect(requestView("openai_chat", { messages: ["hi"] })).toBeNull();
    expect(requestView("openai_chat", { messages: [{ role: "user", content: [{ type: "audio" }] }] })).toBeNull();
    expect(requestView("openai_chat", { messages: [{ role: "user", content: ["hi"] }] })).toBeNull();
    expect(requestView("openai_chat", { messages: [{ role: "user", content: [{ type: "text" }] }] })).toBeNull();
    expect(requestView("openai_chat", { messages: [{ role: "assistant", tool_calls: [{ id: "c1" }] }] })).toBeNull();
    expect(requestView("openai_chat", { messages: [], tools: [42] })).toBeNull();
    expect(requestView("openai_chat", { messages: [], tools: [{}] })).toBeNull();
  });

  it("maps OpenAI Responses messages, parts and tools", () => {
    const v = requestView("openai_responses", {
      instructions: "Answer in French.",
      input: [
        {
          role: "user",
          content: [
            { type: "input_text", text: "Bonjour" },
            { type: "input_image", image_url: "https://example.com/b.png" },
            { type: "input_image" },
            { type: "input_file", filename: "notes.txt" },
            { type: "input_file" },
          ],
        },
        { type: "message", role: "assistant", content: [{ type: "output_text", text: "Salut" }, { type: "refusal", refusal: "Non." }, { type: "refusal" }] },
        { role: "user", content: "Merci" },
        { type: "function_call", name: "translate", arguments: { text: "hi" } },
        { type: "function_call_output", output: { ok: true } },
      ],
      tools: [{ type: "function", name: "translate", description: "Translate text", parameters: { type: "object" } }, { type: "web_search" }],
      max_output_tokens: 100,
    });
    expect(v?.messages).toEqual([
      { role: "system", content: "Answer in French." },
      {
        role: "user",
        content: [
          { type: "text", text: "Bonjour" },
          { type: "image", src: "https://example.com/b.png" },
          { type: "image" },
          { type: "document", name: "notes.txt" },
          { type: "document" },
        ],
      },
      {
        role: "assistant",
        content: [
          { type: "text", text: "Salut" },
          { type: "refusal", text: "Non." },
          { type: "refusal", text: "" },
        ],
      },
      { role: "user", content: [{ type: "text", text: "Merci" }] },
      { role: "assistant", content: [{ type: "tool_call", name: "translate", arguments: { text: "hi" } }] },
      { role: "tool", content: [{ type: "tool_result", content: '{"ok":true}' }] },
    ]);
    expect(v?.tools).toEqual([{ name: "translate", description: "Translate text", schema: { type: "object" } }, { name: "web_search" }]);
    expect(v?.params).toEqual([{ label: "Max output tokens", value: "100" }]);
  });

  it("refuses OpenAI Responses shapes it doesn't know", () => {
    expect(requestView("openai_responses", { input: ["hi"] })).toBeNull();
    expect(requestView("openai_responses", { input: [{ type: "computer_call" }] })).toBeNull();
    expect(requestView("openai_responses", { input: [{ role: "user", content: [{ type: "input_audio" }] }] })).toBeNull();
    expect(requestView("openai_responses", { input: [{ role: "user", content: [null] }] })).toBeNull();
    expect(requestView("openai_responses", { input: [{ role: "user", content: [{ type: "input_text" }] }] })).toBeNull();
    expect(requestView("openai_responses", { input: [], tools: [null] })).toBeNull();
    expect(requestView("openai_responses", { input: [], tools: [{}] })).toBeNull();
  });

  it("reads reasoning blobs it can't decode as hidden", () => {
    const v = requestView("openai_responses", {
      input: [
        { type: "reasoning", summary: ["Plain text summary", 42], encrypted_content: "kpp:reasoning:v1:%%%" },
        { type: "reasoning", summary: [], encrypted_content: `kpp:reasoning:v1:${btoa("[1,2]")}` },
      ],
    });
    expect(v?.messages).toEqual([
      { role: "assistant", content: [{ type: "reasoning", text: "Plain text summary" }] },
      { role: "assistant", content: [{ type: "reasoning", text: "" }] },
    ]);
  });

  it("counts a single tokenised embedding input and ignores other shapes", () => {
    expect(embeddingsView("openai_embeddings", { input: [1, 2, 3] })).toEqual({ inputs: 1 });
    expect(embeddingsView("openai_embeddings", { input: { text: "x" } })).toBeNull();
    expect(embeddingsView("openai_embeddings", "x")).toBeNull();
    expect(embeddingsView("openai_chat", { input: "x" })).toBeNull();
  });
});

describe("replyView, every block", () => {
  it("reads tool results, refusals, images and documents", () => {
    const v = replyView({
      blocks: [
        { tool_call: { name: "lookup", arguments: {} } },
        { tool_result: { call_id: "c1", content: [{ text: { text: "Shipped" } }, { unknown: { x: 1 } }], is_error: true } },
        { tool_result: { content: [] } },
        { refusal: { text: "No." } },
        { refusal: {} },
        { image: { source: { url: "https://example.com/i.png" } } },
        { image: { source: { base64: { media_type: "image/jpeg", data: "/9j/" } } } },
        { image: { source: { base64: {} } } },
        { image: { source: { text: { media_type: "text/plain" } } } },
        { image: { source: { text: {} } } },
        { image: { source: "nowhere" } },
        { image: { source: { file: "x" } } },
        { document: { title: "Invoice", source: { base64: { media_type: "application/pdf", data: "JVBE" } } } },
        { document: { source: { url: "https://example.com/d.pdf" } } },
        { reasoning: "flat" },
      ],
      stop: { other: "guardrail" },
    });
    expect(v?.blocks).toEqual([
      { type: "tool_call", name: "lookup", arguments: {} },
      { type: "tool_result", tool_call_id: "c1", content: [{ type: "text", text: "Shipped" }], is_error: true },
      { type: "tool_result", content: [] },
      { type: "refusal", text: "No." },
      { type: "refusal", text: "" },
      { type: "image", src: "https://example.com/i.png" },
      { type: "image", src: "data:image/jpeg;base64,/9j/", media_type: "image/jpeg" },
      { type: "image", src: "data:application/octet-stream;base64," },
      { type: "image", media_type: "text/plain" },
      { type: "image" },
      { type: "image" },
      { type: "image" },
      { type: "document", name: "Invoice", media_type: "application/pdf" },
      { type: "document" },
      { type: "reasoning", text: "" },
    ]);
    expect(v?.unknown).toEqual([{ x: 1 }]);
    expect(v?.stop).toBe("guardrail");
    expect(v?.model).toBe("");
    expect(v?.id).toBe("");
  });

  it("is null for blocks it doesn't know", () => {
    expect(replyView({ blocks: [{ audio: {} }] })).toBeNull();
    expect(replyView({ blocks: ["text"] })).toBeNull();
    expect(replyView({ blocks: [{}] })).toBeNull();
    expect(replyView({ blocks: [{ text: {} }] })).toBeNull();
    expect(replyView({ blocks: [{ tool_call: { arguments: {} } }] })).toBeNull();
    expect(replyView({ text: "no blocks" })).toBeNull();
  });

  it("names stop reasons it knows and keeps the rest", () => {
    expect(stopLabel("tool_use")).toBe("Tool use");
    expect(stopLabel("length_cap")).toBe("length_cap");
    expect(stopLabel({ stop_sequence: "" })).toBe('Stop sequence ""');
    expect(stopLabel(null)).toBe("Unknown");
    expect(stopLabel({})).toBe("Unknown");
  });
});

describe("requestView and replyView, edge cases", () => {
  it("refuses tool calls without a name in every dialect", () => {
    expect(requestView("anthropic_messages", { messages: [{ role: "assistant", content: [{ type: "tool_use", id: "tu_1", input: {} }] }] })).toBeNull();
    expect(requestView("openai_chat", { messages: [{ role: "assistant", tool_calls: [{ id: "c1", function: { arguments: "{}" } }] }] })).toBeNull();
    expect(requestView("openai_responses", { input: [{ type: "function_call", call_id: "c1", arguments: "{}" }] })).toBeNull();
  });

  it("shows thinking without text, and skips summary parts without text", () => {
    const anthropic = requestView("anthropic_messages", { messages: [{ role: "assistant", content: [{ type: "thinking" }] }] });
    expect(anthropic?.messages[0]?.content).toEqual([{ type: "reasoning", text: "" }]);
    const responses = requestView("openai_responses", {
      input: [{ type: "reasoning", summary: [{ type: "summary_text" }, { type: "summary_text", text: "Kept." }] }],
    });
    expect(responses?.messages).toEqual([{ role: "assistant", content: [{ type: "reasoning", text: "Kept." }] }]);
  });

  it("counts only the signed data it can measure", () => {
    const v = replyView({ blocks: [{ reasoning: { text: ["Thought."], opaque: ["flat", { kind: "signature" }, { kind: "signature", data: "abcdef" }] } }] });
    expect(v?.blocks).toEqual([{ type: "reasoning", text: "Thought.\n\n6 bytes of provider-signed data, not readable." }]);
  });

  it("lets errors other than an unknown shape through", () => {
    const throwing = Object.defineProperty({}, "messages", {
      enumerable: true,
      get: () => {
        throw new TypeError("broken body");
      },
    });
    expect(() => requestView("openai_chat", throwing)).toThrow("broken body");
    const block = Object.defineProperty({}, "text", {
      enumerable: true,
      get: () => {
        throw new TypeError("broken block");
      },
    });
    expect(() => replyView({ blocks: [block] })).toThrow("broken block");
  });
});
