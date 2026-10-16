import { describe, expect, it, vi } from "vitest";
import { load } from "./index";

vi.mock("./library", () => {
  throw new Error("The chunk couldn't be fetched.");
});

describe("loading another language", () => {
  it("stays in English when the texts for it can't be fetched", async () => {
    expect(await load("de")).toEqual({ locale: "en", messages: {}, ui: {}, provider: undefined });
  });
});
