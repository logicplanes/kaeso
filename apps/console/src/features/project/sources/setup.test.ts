import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { SOURCE_KINDS } from "./labels";
import { HTTP_EXAMPLE, KEY_PLACEHOLDER, setupSnippets, type SetupTarget } from "./setup";

const KEY = "sk-kso-src-AbCdEf";
const at = (kind: SetupTarget["kind"], content = false): SetupTarget => ({ kind, ingestUrl: "https://kaeso.example.com/ingest", secret: KEY, content });
const all = (target: SetupTarget) =>
  setupSnippets(target)
    .map((tab) => tab.code)
    .join("\n");

describe("setup snippets", () => {
  it("give every kind lines with the source's key and the install's ingest address", () => {
    for (const kind of SOURCE_KINDS) {
      const tabs = setupSnippets(at(kind));
      expect(tabs.length, kind).toBeGreaterThan(0);
      expect(all(at(kind)), kind).toContain(KEY);
      expect(all(at(kind)), kind).toContain("https://kaeso.example.com/ingest");
    }
  });

  it("set Claude Code's protocol and tool details, in its settings file and as a shell", () => {
    const [settings, shell] = setupSnippets(at("claude_code"));
    const env = (JSON.parse(settings?.code ?? "{}") as { env: Record<string, string> }).env;
    expect(env).toMatchObject({
      CLAUDE_CODE_ENABLE_TELEMETRY: "1",
      OTEL_LOGS_EXPORTER: "otlp",
      OTEL_EXPORTER_OTLP_PROTOCOL: "http/protobuf",
      OTEL_EXPORTER_OTLP_ENDPOINT: "https://kaeso.example.com/ingest/otlp",
      OTEL_EXPORTER_OTLP_HEADERS: `Authorization=Bearer ${KEY}`,
      OTEL_LOG_TOOL_DETAILS: "1",
    });
    expect(env.OTEL_LOG_USER_PROMPTS).toBeUndefined();
    expect(shell?.code).toContain(`export OTEL_EXPORTER_OTLP_HEADERS="Authorization=Bearer ${KEY}"`);
    expect(all(at("claude_code", true))).toContain("OTEL_LOG_USER_PROMPTS");
  });

  it("send Codex's logs to their full path and keep its prompts only with content", () => {
    expect(all(at("codex"))).toContain('endpoint = "https://kaeso.example.com/ingest/otlp/v1/logs", protocol = "binary"');
    expect(all(at("codex"))).toContain("log_user_prompt = false");
    expect(all(at("codex", true))).toContain("log_user_prompt = true");
  });

  it("turn Gemini CLI's prompt logging off unless the source keeps content", () => {
    const [settings] = setupSnippets(at("gemini_cli"));
    expect(JSON.parse(settings?.code ?? "{}")).toMatchObject({
      telemetry: { otlpProtocol: "http", logPrompts: false, otlpHeaders: { Authorization: `Bearer ${KEY}` } },
    });
    expect(all(at("gemini_cli", true))).toContain("GEMINI_TELEMETRY_LOG_PROMPTS=true");
  });

  it("give Antigravity a hook script that sends only the fields Kaeso reads", () => {
    const [script, hooks] = setupSnippets(at("antigravity"));
    expect(script?.code).toContain('KAESO_INGEST="https://kaeso.example.com/ingest/v1/antigravity"');
    expect(script?.code).toContain("transcript_full.jsonl");
    expect(script?.code).not.toContain("content}");
    expect(script?.code).toContain("del(.lastUserInput)");
    expect(script?.code).toContain("map({name})");
    expect(script?.code).not.toContain("args");
    expect(all(at("antigravity", true))).toContain("output_tokens, content, thinking, tool_calls: (.tool_calls // [] | map({name, args}))");
    expect(all(at("antigravity", true))).toContain("{step_index, type, status, created_at, content}");
    expect(all(at("antigravity", true))).not.toContain("del(.lastUserInput)");
    expect(JSON.parse(hooks?.code ?? "{}")).toEqual({
      kaeso: { PostInvocation: [{ type: "command", command: "sh ~/.gemini/config/kaeso-source.sh", timeout: 15 }] },
    });
  });

  it("point gateways and collectors at the endpoint they post to", () => {
    expect(all(at("litellm"))).toContain("endpoint: https://kaeso.example.com/ingest/v1/litellm");
    expect(all(at("openrouter"))).toContain("https://kaeso.example.com/ingest/otlp/v1/traces");
    expect(all(at("opentelemetry"))).toContain("Authorization=Bearer%20");
    expect(all(at("http"))).toContain("curl https://kaeso.example.com/ingest/v1/events");
    expect(all(at("litellm"))).toContain("DEFAULT_BATCH_SIZE=50");
    expect(all(at("opentelemetry"))).toContain("receivers: [otlp]");
    expect(all(at("codex"))).toContain("# Merge into the [otel] table");
  });

  it("send the HTTP example the gateway's tests take", () => {
    // Tests run from the console's folder.
    const fixture = resolve(process.cwd(), "../../crates/kaeso-server/tests/server/fixtures/sources/http-example.json");
    expect(HTTP_EXAMPLE).toEqual(JSON.parse(readFileSync(fixture, "utf8")));
    expect(all(at("http"))).toContain(`-d '${JSON.stringify(HTTP_EXAMPLE)}'`);
  });

  it("carry the placeholder where the key is no longer shown", () => {
    expect(all({ ...at("codex"), secret: KEY_PLACEHOLDER })).toContain("Bearer <your source key>");
  });
});
