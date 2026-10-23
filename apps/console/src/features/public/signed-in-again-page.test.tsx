import { afterEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";
import { announceConfirmed, onConfirmed, type Confirmed } from "~/auth/confirm-channel";
import { renderApp } from "~/test/render";

afterEach(() => vi.restoreAllMocks());

function listen() {
  const heard: Confirmed[] = [];
  const stop = onConfirmed((c) => heard.push(c));
  return { heard, stop };
}

describe("the window that confirms a sign-in", () => {
  it("tells the console the sign-in is confirmed and closes", async () => {
    const close = vi.spyOn(window, "close").mockImplementation(() => undefined);
    const { heard, stop } = listen();
    await renderApp("/signed-in-again");
    expect(await screen.findByText("You can close this window and continue in Kaeso.")).toBeInTheDocument();
    await waitFor(() => expect(heard).toEqual([{ error: null }]));
    expect(close).toHaveBeenCalled();
    stop();
  });

  it("passes on why the identity provider didn't confirm, and says it in the window", async () => {
    vi.spyOn(window, "close").mockImplementation(() => undefined);
    const { heard, stop } = listen();
    await renderApp("/signed-in-again#sso_error=cannot_confirm");
    expect(await screen.findByRole("heading", { name: "Not confirmed" })).toBeInTheDocument();
    expect(
      screen.getByText("The identity provider can't confirm a recent sign-in; use your password or an email link. Close this window and try again in Kaeso."),
    ).toBeInTheDocument();
    await waitFor(() => expect(heard).toEqual([{ error: "cannot_confirm" }]));
    stop();
  });
});

describe("the confirmation channel", () => {
  it("ignores messages that aren't a confirmation", async () => {
    const { heard, stop } = listen();
    const other = new BroadcastChannel("kaeso-sign-in-again");
    other.postMessage("hello");
    other.postMessage({ error: 42 });
    other.close();
    await waitFor(() => expect(heard).toEqual([{ error: null }]));
    stop();
  });

  it("does nothing where the browser has no channels", () => {
    vi.stubGlobal("BroadcastChannel", undefined);
    const stop = onConfirmed(() => undefined);
    expect(stop).toBeTypeOf("function");
    expect(() => announceConfirmed(null)).not.toThrow();
    stop();
    vi.unstubAllGlobals();
  });
});
