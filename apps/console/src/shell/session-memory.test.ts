import { describe, expect, it } from "vitest";
import { queryClient } from "~/query";
import { recall, remember } from "./session-memory";

describe("session memory", () => {
  it("keeps a value for the session and forgets it when the session's data is cleared", () => {
    expect(recall(["add-models", "provider"])).toBeUndefined();
    remember(["add-models", "provider"], "groq");
    expect(recall<string>(["add-models", "provider"])).toBe("groq");
    queryClient.clear();
    expect(recall(["add-models", "provider"])).toBeUndefined();
  });
});
