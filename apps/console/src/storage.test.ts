import { afterEach, describe, expect, it, vi } from "vitest";
import { readStored, readStoredJson, writeStored } from "./storage";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("browser storage", () => {
  it("keeps and removes values in local and session storage", () => {
    expect(writeStored("kaeso:theme", "dark")).toBe(true);
    expect(readStored("kaeso:theme")).toBe("dark");
    writeStored("kaeso:invite", "inv_tok", "session");
    expect(readStored("kaeso:invite", "session")).toBe("inv_tok");
    expect(window.localStorage.getItem("kaeso:invite")).toBeNull();
    writeStored("kaeso:invite", null, "session");
    expect(readStored("kaeso:invite", "session")).toBeNull();
  });

  it("reads JSON and ignores what isn't", () => {
    writeStored("kaeso:last", JSON.stringify({ org: "org_1" }));
    expect(readStoredJson("kaeso:last")).toEqual({ org: "org_1" });
    writeStored("kaeso:last", "{broken");
    expect(readStoredJson("kaeso:last")).toBeNull();
    expect(readStoredJson("kaeso:missing")).toBeNull();
  });

  it("carries on when the browser blocks storage", () => {
    vi.spyOn(window, "localStorage", "get").mockImplementation(() => {
      throw new DOMException("blocked", "SecurityError");
    });
    expect(readStored("kaeso:theme")).toBeNull();
    expect(readStoredJson("kaeso:theme")).toBeNull();
    expect(writeStored("kaeso:theme", "dark")).toBe(false);
  });
});
