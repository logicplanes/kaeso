import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { setFormatTimeZone } from "@logicplanes/kaeso-ui/format";
import { ENDPOINTS, logsParams, toolCallParams } from "./logs";

describe("request log filters", () => {
  it("offer every endpoint the gateway's filter takes", () => {
    const doc = JSON.parse(readFileSync(join(__dirname, "../../../../../openapi.json"), "utf8")) as {
      components: { schemas: { LoggedEndpoint: { enum: string[] } } };
    };
    expect([...ENDPOINTS].sort()).toEqual([...doc.components.schemas.LoggedEndpoint.enum].sort());
  });

  it("send the alias, fallback, endpoint and cost filters as the gateway names them", () => {
    expect(logsParams({ alias: "fast", fell_back: true, endpoint: "responses", min_cost: "0.5" }, 0)).toMatchObject({
      alias: "fast",
      fell_back: true,
      endpoint: "responses",
      min_cost: "0.5",
    });
    expect(logsParams({ fell_back: false }, 0)).not.toHaveProperty("fell_back");
  });
});

describe("the request log's window", () => {
  afterEach(() => setFormatTimeZone(undefined));

  it("reads days in the install's time zone, and Analytics' links as UTC days", () => {
    setFormatTimeZone("America/Los_Angeles");
    expect(logsParams({ from: "2026-10-12", to: "2026-10-12" })).toMatchObject({
      from: "2026-10-12T07:00:00.000Z",
      to: "2026-10-13T07:00:00.000Z",
    });
    expect(logsParams({ from: "2026-10-12", to: "2026-10-13", utc: true, failed: true })).toEqual({
      from: "2026-10-12T00:00:00.000Z",
      to: "2026-10-14T00:00:00.000Z",
      failed: true,
    });
    expect(toolCallParams({ from: "2026-10-12", to: "2026-10-12", utc: true })).toMatchObject({
      from: "2026-10-12T00:00:00.000Z",
      to: "2026-10-13T00:00:00.000Z",
    });
  });
});
