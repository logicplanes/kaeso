import { describe, expect, it } from "vitest";
import { logContent, logEntry } from "~/test/fixtures/project-logs";
import { playgroundStart } from "./playground-start";

const entry = (ingress_dialect: string, request: unknown, more: { provider?: string; model?: string } = {}) =>
  logEntry({ ingress_dialect, provider: "openai", model: "gpt-5.4-mini", ...more, content: logContent(request, null) });

describe("playgroundStart", () => {
  it("takes an Anthropic request's model, system prompt, settings and last user message", () => {
    const start = playgroundStart(
      entry(
        "anthropic_messages",
        {
          model: "claude-sonnet-4-5",
          system: [{ type: "text", text: "You are terse." }],
          max_tokens: 512,
          temperature: 0.3,
          top_p: 0.9,
          output_config: { effort: "high" },
          messages: [
            { role: "user", content: "First question" },
            { role: "assistant", content: "First answer" },
            { role: "user", content: [{ type: "text", text: "Where is order 7?" }] },
          ],
        },
        { provider: "anthropic", model: "claude-sonnet-4-5" },
      ),
    );
    expect(start).toEqual({
      search: { models: "anthropic/claude-sonnet-4-5", system: "You are terse.", temperature: 0.3, top_p: 0.9, max_tokens: 512, effort: "high" },
      prompt: "Where is order 7?",
      systemLeftOut: false,
    });
  });

  it("joins a Chat Completions request's system and developer messages and keeps only the text of the last prompt", () => {
    const start = playgroundStart(
      entry("openai_chat", {
        model: "gpt-5.4-mini",
        max_tokens: 100,
        max_completion_tokens: 200,
        reasoning_effort: "low",
        temperature: 5,
        messages: [
          { role: "system", content: "Be kind." },
          { role: "developer", content: [{ type: "text", text: "Answer in German." }] },
          {
            role: "user",
            content: [
              { type: "text", text: "What is this?" },
              { type: "image_url", image_url: { url: "https://example.com/a.png" } },
            ],
          },
        ],
      }),
    );
    expect(start?.search).toEqual({ models: "openai/gpt-5.4-mini", system: "Be kind.\n\nAnswer in German.", max_tokens: 200, effort: "low" });
    expect(start?.prompt).toBe("What is this?");
  });

  it("reads a Responses request's instructions, reasoning and string input", () => {
    const start = playgroundStart(
      entry("openai_responses", {
        model: "gpt-5.4-mini",
        instructions: "Short answers.",
        input: "Summarise the log.",
        max_output_tokens: 300,
        reasoning: { effort: "medium", summary: "detailed" },
      }),
    );
    expect(start).toEqual({
      search: { models: "openai/gpt-5.4-mini", system: "Short answers.", max_tokens: 300, effort: "medium", summary: "detailed" },
      prompt: "Summarise the log.",
      systemLeftOut: false,
    });
  });

  it("reads a Gemini request's system instruction and generation settings", () => {
    const start = playgroundStart(
      entry(
        "gemini",
        {
          systemInstruction: { parts: [{ text: "Reply in haiku." }] },
          contents: [{ role: "user", parts: [{ text: "Autumn?" }] }],
          generationConfig: { temperature: 1.2, topP: 0.5, maxOutputTokens: 64, thinkingConfig: { thinkingLevel: "low" } },
        },
        { provider: "google", model: "gemini-3-pro" },
      ),
    );
    expect(start).toEqual({
      search: { models: "google/gemini-3-pro", system: "Reply in haiku.", temperature: 1.2, top_p: 0.5, max_tokens: 64, effort: "low" },
      prompt: "Autumn?",
      systemLeftOut: false,
    });
  });

  it("leaves out a system prompt longer than the Playground takes, and has no prompt without a user message", () => {
    const start = playgroundStart(entry("openai_chat", { model: "m", messages: [{ role: "system", content: "x".repeat(8001) }] }));
    expect(start).toEqual({ search: { models: "openai/gpt-5.4-mini" }, prompt: "", systemLeftOut: true });
    const bare = playgroundStart(entry("gemini", { contents: [{ role: "user", parts: [{ text: "Hi" }] }] }));
    expect(bare).toEqual({ search: { models: "openai/gpt-5.4-mini" }, prompt: "Hi", systemLeftOut: false });
  });

  it("offers nothing for a request it can't read or that reached no model", () => {
    expect(playgroundStart(entry("openai_embeddings", { input: "hi" }))).toBeNull();
    expect(playgroundStart(entry("openai_chat", { messages: [{ role: "robot", content: "?" }] }))).toBeNull();
    expect(playgroundStart(entry("openai_chat", "not an object"))).toBeNull();
    expect(playgroundStart(entry("openai_chat", { messages: [] }, { model: "" }))).toBeNull();
    expect(playgroundStart(logEntry({ ingress_dialect: "openai_chat" }))).toBeNull();
  });
});
