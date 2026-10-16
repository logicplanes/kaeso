import { describe, expect, it } from "vitest";
import { sendTraffic } from "./traffic.mjs";

const ok = () => Promise.resolve();
const refusedByGateway = () => Promise.reject(new Error("429 Rate limit reached"));

describe("the seed's live traffic", () => {
  it("counts the requests meant to be refused", async () => {
    await expect(sendTraffic([ok, ok, ok], [refusedByGateway, refusedByGateway])).resolves.toEqual({ sent: 5, refused: 2 });
  });

  it("stops the seed when a request that should go through fails", async () => {
    const down = () => Promise.reject(new Error("Connection error."));
    await expect(sendTraffic([ok, down, down], [refusedByGateway])).rejects.toThrow("seed: 2 of 4 requests didn't go as planned; the first: Connection error.");
  });

  it("stops the seed when a request meant to be refused goes through", async () => {
    await expect(sendTraffic([ok], [ok])).rejects.toThrow("seed: 1 of 2 requests didn't go as planned; the first: a request meant to be refused went through");
  });
});
