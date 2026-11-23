import { execFileSync, spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { setupSnippets } from "./setup";

const hasJq = spawnSync("jq", ["--version"]).status === 0;

/** Runs the hook as Antigravity would, with a curl that keeps what it was sent; returns each body it got. */
function runHook(
  dir: string,
  hook: object,
  content: boolean,
  runs = 1,
): { answers: string[]; bodies: { hook: Record<string, unknown>; steps: Record<string, unknown>[] }[] } {
  const [script] = setupSnippets({ kind: "antigravity", ingestUrl: "https://kaeso.example.com/ingest", secret: "sk-kso-src-test", content });
  const bin = join(dir, "bin");
  mkdirSync(bin, { recursive: true });
  const sent = join(dir, "sent");
  mkdirSync(sent, { recursive: true });
  writeFileSync(join(bin, "curl"), `#!/bin/sh\ncat > "${sent}/$(date +%s%N)"\n`);
  chmodSync(join(bin, "curl"), 0o755);
  writeFileSync(join(dir, "hook.sh"), script?.code ?? "");
  const answers: string[] = [];
  for (let i = 0; i < runs; i++) {
    const before = existsSync(sent) ? execFileSync("ls", [sent]).toString().split("\n").filter(Boolean).length : 0;
    answers.push(
      execFileSync("sh", [join(dir, "hook.sh")], {
        input: JSON.stringify(hook),
        env: { PATH: `${bin}:${process.env.PATH ?? ""}`, TMPDIR: dir },
      }).toString(),
    );
    // The hook sends in the background; wait until its curl has run, or a moment when nothing is new.
    const until = Date.now() + 3_000;
    while (Date.now() < until && execFileSync("ls", [sent]).toString().split("\n").filter(Boolean).length === before) execFileSync("sleep", ["0.05"]);
  }
  const files = execFileSync("ls", [sent]).toString().split("\n").filter(Boolean).sort();
  return {
    answers,
    bodies: files.map((f) => JSON.parse(readFileSync(join(sent, f), "utf8")) as { hook: Record<string, unknown>; steps: Record<string, unknown>[] }),
  };
}

function transcript(dir: string, steps: number): string {
  const logs = join(dir, "brain", "conv-1", ".system_generated", "logs");
  mkdirSync(logs, { recursive: true });
  const lines = Array.from({ length: steps }, (_, i) =>
    JSON.stringify(
      i % 2 === 0
        ? {
            type: "PLANNER_RESPONSE",
            step_index: i,
            created_at: "2026-10-15T11:00:00Z",
            model: "gemini-4-pro",
            input_tokens: 100,
            output_tokens: 10,
            content: "secret plan",
          }
        : { type: "TOOL_RESULT", step_index: i, content: "file contents" },
    ),
  );
  writeFileSync(join(logs, "transcript_full.jsonl"), `${lines.join("\n")}\n`);
  writeFileSync(join(logs, "transcript.jsonl"), "");
  return join(logs, "transcript.jsonl");
}

describe.skipIf(!hasJq)("the Antigravity hook", () => {
  it("answers at once and sends each model call once, without the prompt or content unless asked", () => {
    const dir = mkdtempSync(join(tmpdir(), "kaeso-hook-"));
    const hook = { conversationId: "conv-1", transcriptPath: transcript(dir, 4), modelName: "gemini-4-pro", lastUserInput: "my private prompt" };
    const { answers, bodies } = runHook(dir, hook, false, 2);
    expect(answers).toEqual(["{}", "{}"]);
    expect(bodies).toHaveLength(1);
    expect(bodies[0]?.hook).toEqual({ conversationId: "conv-1", transcriptPath: hook.transcriptPath, modelName: "gemini-4-pro" });
    expect(bodies[0]?.steps.map((s) => s.step_index)).toEqual([0, 2]);
    expect(bodies[0]?.steps[0]).not.toHaveProperty("content");
    expect(readFileSync(join(dir, "kaeso-antigravity", "conv-1"), "utf8")).toBe("2");
  });

  it("keeps the prompt and the content when the source keeps content", () => {
    const dir = mkdtempSync(join(tmpdir(), "kaeso-hook-"));
    const hook = { conversationId: "conv-1", transcriptPath: transcript(dir, 2), lastUserInput: "my prompt" };
    const { bodies } = runHook(dir, hook, true);
    expect(bodies[0]?.hook.lastUserInput).toBe("my prompt");
    expect(bodies[0]?.steps[0]?.content).toBe("secret plan");
  });

  it("sends a long conversation 1,000 model calls at a time", () => {
    const dir = mkdtempSync(join(tmpdir(), "kaeso-hook-"));
    const hook = { conversationId: "conv-1", transcriptPath: transcript(dir, 2_400) };
    const { bodies } = runHook(dir, hook, false, 3);
    expect(bodies.map((b) => b.steps.length)).toEqual([1_000, 200]);
  });

  it("sends a model call's tool calls and the steps that ran them, their arguments and results only with content", () => {
    for (const content of [false, true]) {
      const dir = mkdtempSync(join(tmpdir(), "kaeso-antigravity-"));
      const logs = join(dir, "brain", "conv-1", ".system_generated", "logs");
      mkdirSync(logs, { recursive: true });
      const lines = [
        { type: "USER_INPUT", step_index: 0, content: "<USER_REQUEST>close it</USER_REQUEST>" },
        {
          type: "PLANNER_RESPONSE",
          step_index: 1,
          model: "gemini-3.8-flash-low",
          input_tokens: 10,
          output_tokens: 2,
          thinking: "use linear",
          tool_calls: [{ name: "mcp_linear_close_issue", args: { id: "ENG-1" } }],
        },
        { type: "MCP_TOOL", step_index: 2, status: "DONE", created_at: "2026-10-15T11:00:02Z", content: "closed" },
        { type: "CHECKPOINT", step_index: 3 },
      ];
      writeFileSync(join(logs, "transcript_full.jsonl"), `${lines.map((l) => JSON.stringify(l)).join("\n")}\n`);
      writeFileSync(join(logs, "transcript.jsonl"), "");
      const { bodies } = runHook(dir, { conversationId: "conv-1", transcriptPath: join(logs, "transcript.jsonl") }, content);
      const steps = bodies[0]?.steps ?? [];
      expect(steps.map((s) => s.step_index)).toEqual([1, 2]);
      expect(steps[0]?.tool_calls).toEqual(content ? [{ name: "mcp_linear_close_issue", args: { id: "ENG-1" } }] : [{ name: "mcp_linear_close_issue" }]);
      expect(steps[0]?.thinking).toBe(content ? "use linear" : undefined);
      expect(steps[1]).toEqual({
        step_index: 2,
        type: "MCP_TOOL",
        status: "DONE",
        created_at: "2026-10-15T11:00:02Z",
        ...(content ? { content: "closed" } : {}),
      });
    }
  });

  it("does nothing without a conversation or a transcript", () => {
    const dir = mkdtempSync(join(tmpdir(), "kaeso-hook-"));
    const { answers, bodies } = runHook(dir, { transcriptPath: join(dir, "missing", "transcript.jsonl") }, false);
    expect(answers).toEqual(["{}"]);
    expect(bodies).toEqual([]);
  });
});
