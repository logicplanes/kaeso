import { afterEach, describe, expect, it, vi } from "vitest";
import { readStored, readStoredJson, takeStored, writeStored } from "./storage";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("browser storage", () => {
  it("keeps and takes values in local and session storage", () => {
    expect(writeStored("kaeso:theme", "dark")).toBe(true);
    expect(readStored("kaeso:theme")).toBe("dark");
    writeStored("kaeso:return", "/orgs/org_1", "session");
    expect(takeStored("kaeso:return", "session")).toBe("/orgs/org_1");
    expect(readStored("kaeso:return", "session")).toBeNull();
    expect(window.localStorage.getItem("kaeso:return")).toBeNull();
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
    expect(takeStored("kaeso:theme")).toBeNull();
  });
});
