import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { ENDPOINTS, logsParams } from "./logs";

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
