import { describe, expect, it, vi } from "vitest";
import { playgroundModel } from "~/test/fixtures/project-logs";
import { columnsOf, conversationJson, conversationMarkdown, exportName, saveText, type Conversation } from "./export";
import { newResponse, type ResponseState, type Turn } from "./responses";

const reply = (overrides: Partial<ResponseState>): ResponseState => ({ ...newResponse(1000), status: "completed", endedAt: 2200, ...overrides });

const text: Turn = {
  id: "t1",
  prompt: "Where is order 7?",
  files: [{ id: "f1", name: "receipt.png", type: "image/png", data: "data:image/png;base64,AAAA" }],
  response: reply({
    text: "In transit.",
    reasoning: "Look it up.",
    firstTokenAt: 1400,
    usage: { input_tokens: 120, output_tokens: 48, reasoning_tokens: 0, cached_tokens: 0, cache_write_tokens: 0, cost: "0.00042" },
    requestId: "req_01text",
  }),
};
const failed: Turn = { id: "t2", prompt: "And order 8?", response: reply({ status: "failed", error: "Rate limit reached.", requestId: "req_01fail" }) };
const stopped: Turn = { id: "t3", prompt: "Tell me more", response: reply({ status: "stopped", text: "Well" }) };
const image: Turn = {
  id: "t4",
  mode: "image",
  prompt: "A lighthouse",
  response: reply({
    media: [
      { src: "blob:x", name: "image-1.png", label: "Image 1" },
      { src: "blob:y", name: "image-2.png" },
    ],
    cost: "0.04",
  }),
};
const decision: Turn = {
  id: "t5",
  mode: "decision",
  prompt: "Classify",
  questions: [
    { name: "is_invoice", kind: "predicate" },
    { kind: "choice", instructions: "Which kind?", options: [{ value: "receipt" }, { value: "bill" }] },
    { kind: "choice", options: [] },
    { name: "urgency", kind: "score" },
    { name: "where", kind: "box" },
    { name: "why", kind: "predicate" },
    { name: "how", kind: "predicate" },
    { name: "who", kind: "predicate" },
  ],
  response: reply({
    answers: [
      { kind: "predicate", probability: 0.82 },
      { kind: "choice", option: 1 },
      { kind: "choice", option: 3 },
      { kind: "score", score: 2 },
      { kind: "box", top: 1, bottom: 2, left: 3, right: 4 },
      { kind: "refusal", reason: "unclear" },
      { kind: "refusal" },
      null,
    ],
  }),
};

const conversation = (): Conversation => ({
  project: "Checkout Web!",
  parameters: { temperature: 0.2, system: "", max_output_tokens: undefined },
  columns: [
    { ref: "openai/gpt-5.4-mini", label: "GPT-5.4 mini", model: playgroundModel(), turns: [text, failed, stopped] },
    { ref: "openai/gpt-image-2", label: "GPT Image 2", model: null, turns: [image, decision] },
  ],
  at: new Date(2026, 10, 15, 14, 30),
});

describe("the playground export", () => {
  it("names the file after the project and the time", () => {
    expect(exportName(conversation(), "md")).toBe("playground-checkout-web-2026-11-15-1430.md");
    expect(exportName({ ...conversation(), project: "!!" }, "json")).toBe("playground-project-2026-11-15-1430.json");
    expect(exportName({ ...conversation(), project: undefined }, "json")).toBe("playground-project-2026-11-15-1430.json");
  });

  it("writes each column's turns as Markdown with their figures, failures and media by name", () => {
    const md = conversationMarkdown(conversation());
    expect(md).toContain("# Playground conversation");
    expect(md).toContain("Parameters: `temperature=0.2`");
    expect(md).toContain("## GPT-5.4 mini (`openai/gpt-5.4-mini`)");
    expect(md).toContain("### Turn 1\n\n**Prompt**\n\nWhere is order 7?\n\nAttachments: receipt.png (image/png)");
    expect(md).toContain("**Reply**\n\nIn transit.");
    expect(md).toMatch(/_first token .* · took .* · 120 in · 48 out · \$0\.000420 · request req_01text_/);
    expect(md).toContain("Failed: Rate limit reached.");
    expect(md).toContain("request req_01fail");
    expect(md).toContain("Well\nStopped before the model finished.");
    expect(md).toContain("- Image 1 (image-1.png)\n- image-2.png (image-2.png)");
    expect(md).not.toContain("blob:");
    expect(md).toContain("- is_invoice: yes, probability 0.82");
    expect(md).toContain("- Which kind?: bill");
    expect(md).toContain("- 3: 3");
    expect(md).toContain("- urgency: 2");
    expect(md).toContain("- where: top 1, bottom 2, left 3, right 4");
    expect(md).toContain("- why: refused: unclear");
    expect(md).toContain("- how: refused");
    expect(md).toContain("- who: no answer");
  });

  it("leaves out parameters when the address sets none, and what a turn doesn't have", () => {
    const quiet: Turn[] = [
      { id: "w", prompt: "Still thinking?", response: newResponse(1000) },
      { id: "x", prompt: "Nothing?", response: reply({ status: "failed", error: null }) },
      { id: "y", mode: "decision", prompt: "Asked nothing", response: reply({}) },
    ];
    const md = conversationMarkdown({ ...conversation(), parameters: {}, columns: [{ ref: "m", label: "M", model: null, turns: quiet }] });
    expect(md).not.toContain("Parameters");
    expect(md).toContain("Still thinking?\n\n**Reply**\n\nNo reply.\n\n### Turn 2");
    expect(md).toContain("Failed: \n");
    expect(md).toContain("Asked nothing\n\n**Reply**\n\nNo reply.\n\n_took ");
  });

  it("takes the page's columns that have turns, named by their model or its address", () => {
    const model = playgroundModel();
    expect(
      columnsOf(
        ["openai/gpt-5.4-mini", "gone/model", "openai/empty", "openai/missing"],
        [model, null, model],
        [{ turns: [text] }, { turns: [failed] }, { turns: [] }],
      ),
    ).toEqual([
      { ref: "openai/gpt-5.4-mini", label: model.display_name, model, turns: [text] },
      { ref: "gone/model", label: "gone/model", model: null, turns: [failed] },
    ]);
  });

  it("writes the same as JSON, with each request id and no media bytes", () => {
    const doc = JSON.parse(conversationJson(conversation())) as {
      exported_at: string;
      columns: { model: string; turns: { mode: string; attachments: unknown[]; questions?: unknown[]; reply: Record<string, unknown> }[] }[];
    };
    expect(doc.exported_at).toBe(new Date(2026, 10, 15, 14, 30).toISOString());
    const [chat, media] = doc.columns;
    expect(chat?.model).toBe("openai/gpt-5.4-mini");
    expect(chat?.turns[0]).toMatchObject({
      mode: "text",
      attachments: [{ name: "receipt.png", type: "image/png" }],
      reply: { status: "completed", request_id: "req_01text", text: "In transit.", input_tokens: 120, output_tokens: 48, cost: "0.00042", duration_ms: 1200 },
    });
    expect(chat?.turns[1]?.reply).toMatchObject({ status: "failed", error: "Rate limit reached." });
    expect(media?.turns[0]?.reply.media).toEqual([
      { name: "image-1.png", label: "Image 1" },
      { name: "image-2.png", label: null },
    ]);
    expect(media?.turns[1]?.questions).toHaveLength(8);
    expect(chat?.turns[0]?.reply.reasoning).toBe("Look it up.");
    expect((JSON.parse(conversationJson({ ...conversation(), project: undefined })) as { project: unknown }).project).toBeNull();
    expect(JSON.stringify(doc)).not.toContain("base64");
  });

  it("saves the text through the browser", () => {
    const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => undefined);
    saveText("a.md", "# hi", "text/markdown");
    expect(click).toHaveBeenCalledTimes(1);
    click.mockRestore();
  });
});
