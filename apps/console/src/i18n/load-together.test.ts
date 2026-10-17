import { describe, expect, it, vi } from "vitest";
import { load } from "./index";

const gate = vi.hoisted(() => {
  let open!: () => void;
  const promise = new Promise<void>((resolve) => (open = resolve));
  return { promise, open };
});
const loadUiMessages = vi.hoisted(() => vi.fn(async () => ({ "dialog.close": "Schließen" })));

vi.mock("./locales/de", async () => {
  await gate.promise;
  return { default: { "signIn.title": "Anmelden" } };
});
vi.mock("./library", () => ({ LocaleProvider: () => null, loadUiMessages }));

describe("loading German at start", () => {
  it("asks for the library's texts while the console's own are still on their way, and uses both", async () => {
    const loading = load("de");
    await vi.waitFor(() => expect(loadUiMessages).toHaveBeenCalledWith("de"));
    gate.open();
    const loaded = await loading;
    expect(loaded.locale).toBe("de");
    expect(loaded.messages).toEqual({ "signIn.title": "Anmelden" });
    expect(loaded.ui).toEqual({ "dialog.close": "Schließen" });
    expect(loaded.provider).toBeTypeOf("function");
  });
});
