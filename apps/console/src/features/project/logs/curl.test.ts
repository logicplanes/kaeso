import { describe, expect, it } from "vitest";
import { curlCommand } from "./curl";

const ORIGIN = "https://kaeso.example.com";
const entry = (endpoint: string, request: unknown, more: { ingress_dialect?: string; requested_model?: string } = {}) => ({
  endpoint,
  ingress_dialect: more.ingress_dialect ?? "openai_chat",
  requested_model: more.requested_model ?? "gpt-5",
  content: { request, reply: null },
});

describe("curlCommand", () => {
  it("sends the logged body to the endpoint it was sent to, with the key from the environment", () => {
    const body = { model: "gpt-5", messages: [{ role: "user", content: "It's fine" }] };
    expect(curlCommand(entry("chat/completions", body), ORIGIN)).toBe(
      [
        "curl 'https://kaeso.example.com/v1/chat/completions'",
        '  -H "Authorization: Bearer $KAESO_API_KEY"',
        "  -H 'Content-Type: application/json'",
        `  -d '{"model":"gpt-5","messages":[{"role":"user","content":"It'\\''s fine"}]}'`,
      ].join(" \\\n"),
    );
  });

  it("reads a stream as it comes and builds Gemini's path from the model the app named", () => {
    expect(curlCommand(entry("responses", { model: "gpt-5", input: "Hi", stream: true }), ORIGIN)).toMatch(
      /^curl -N 'https:\/\/kaeso\.example\.com\/v1\/responses'/,
    );
    expect(
      curlCommand(entry("models:streamGenerateContent", { contents: [] }, { ingress_dialect: "gemini", requested_model: "gemini-2.5-pro" }), ORIGIN),
    ).toMatch(/^curl -N 'https:\/\/kaeso\.example\.com\/v1beta\/models\/gemini-2\.5-pro:streamGenerateContent\?alt=sse'/);
    expect(curlCommand(entry("models:generateContent", { contents: [] }, { ingress_dialect: "gemini", requested_model: "gemini-2.5-pro" }), ORIGIN)).toMatch(
      /^curl 'https:\/\/kaeso\.example\.com\/v1beta\/models\/gemini-2\.5-pro:generateContent'/,
    );
    expect(curlCommand(entry("decisions", { model: "m" }, { ingress_dialect: "gemini_decisions" }), ORIGIN)).toContain("/v1beta/decisions'");
    expect(curlCommand(entry("decisions", { model: "m" }, { ingress_dialect: "openai_decisions" }), ORIGIN)).toContain("/v1/decisions'");
    expect(curlCommand(entry("messages", { model: "claude-haiku-4-5" }), ORIGIN)).toContain("/v1/messages'");
  });

  it("offers nothing for a request it can't send again whole", () => {
    const image = {
      model: "gpt-5",
      messages: [
        { role: "user", content: [{ type: "image_url", image_url: { url: { kaeso_media: { media_type: "image/png", bytes: 9, reason: "not_kept" } } } }] },
      ],
    };
    expect(curlCommand(entry("chat/completions", image), ORIGIN)).toBeNull();
    for (const endpoint of [
      "audio/transcriptions",
      "images/edits",
      "videos",
      "videos/remix",
      "videos/completed",
      "flows/image/completed",
      "text-to-speech",
      "systemone",
    ]) {
      expect(curlCommand(entry(endpoint, { model: "m" }), ORIGIN), endpoint).toBeNull();
    }
    expect(curlCommand(entry("models:generateContent", { contents: [] }, { ingress_dialect: "gemini", requested_model: "" }), ORIGIN)).toBeNull();
    expect(curlCommand(entry("models:unknownMethod", {}, { ingress_dialect: "gemini" }), ORIGIN)).toBeNull();
    expect(curlCommand(entry("chat/completions", "a body that isn't JSON"), ORIGIN)).toBeNull();
    expect(curlCommand({ ...entry("chat/completions", null), content: null }, ORIGIN)).toBeNull();
  });
});
