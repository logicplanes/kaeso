import { describe, expect, it } from "vitest";
import { SOURCE_KINDS, eventsByDay, kindHint, kindName, refusalText, statusLabel, statusTone, totals } from "./labels";

const day = (d: string, requests: number, tool_calls = 0, refused = 0, duplicates = 0) => ({ day: d, requests, tool_calls, refused, duplicates });

describe("source labels", () => {
  it("name and describe every kind", () => {
    expect(SOURCE_KINDS.map(kindName)).toEqual(["Claude Code", "Codex", "Gemini CLI", "Antigravity", "LiteLLM", "OpenRouter", "OpenTelemetry", "HTTP"]);
    expect(new Set(SOURCE_KINDS.map(kindHint)).size).toBe(SOURCE_KINDS.length);
    expect(kindName("carrier_pigeon")).toBe("carrier_pigeon");
  });

  it("put each status in a word and a tone", () => {
    const statuses = ["waiting", "connected", "idle", "disabled", "archived"] as const;
    expect(statuses.map(statusLabel)).toEqual(["Waiting", "Connected", "Idle", "Disabled", "Archived"]);
    expect(statuses.map(statusTone)).toEqual(["info", "success", "warning", "neutral", "neutral"]);
  });

  it("say why Kaeso refused, for every code it keeps", () => {
    const codes = [
      "no_model",
      "no_tool",
      "no_id",
      "too_many_tokens",
      "cost_too_high",
      "bad_status",
      "too_old",
      "in_future",
      "nobody",
      "unreadable",
      "inactive",
    ];
    const texts = codes.map(refusalText);
    expect(new Set(texts).size).toBe(codes.length);
    expect(texts.every((x) => x.endsWith("."))).toBe(true);
    expect(refusalText("something_new")).toBe("something_new");
  });

  it("sum the recent days and lay them out day by day", () => {
    const s = { activity: [day("2026-10-13", 3, 1, 2), day("2026-10-15", 4, 0, 0, 5)] };
    expect(totals(s)).toEqual({ requests: 7, toolCalls: 1, refused: 2, duplicates: 5 });
    const days = eventsByDay(s, "2026-10-15", 4);
    expect(days).toEqual([
      { day: "2026-10-12", events: 0 },
      { day: "2026-10-13", events: 4 },
      { day: "2026-10-14", events: 0 },
      { day: "2026-10-15", events: 4 },
    ]);
    expect(eventsByDay({ activity: [] }, "2026-10-15")).toHaveLength(14);
  });
});
