import { afterEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import { renderToString } from "react-dom/server";
import { hydrateRoot } from "react-dom/client";
import { Kbd, KbdSequence } from "./kbd.js";

const onPlatform = (platform: string) => Object.defineProperty(navigator, "platform", { configurable: true, value: platform });

afterEach(() => {
  delete (navigator as unknown as Record<string, unknown>).platform;
  document.body.innerHTML = "";
});

describe("Kbd", () => {
  it("hydrates a server render without a mismatch, then shows the Mac key", async () => {
    const container = document.createElement("div");
    onPlatform("MacIntel");
    container.innerHTML = renderToString(<Kbd>mod</Kbd>);
    document.body.append(container);
    expect(container).toHaveTextContent("Ctrl");
    const onRecoverableError = vi.fn();
    const root = await act(async () => hydrateRoot(container, <Kbd>mod</Kbd>, { onRecoverableError }));
    expect(onRecoverableError).not.toHaveBeenCalled();
    expect(screen.getByText("⌘")).toHaveAttribute("aria-hidden", "true");
    expect(screen.getByText("Command")).toHaveClass("sr-only");
    act(() => root.unmount());
  });

  it("reads a key as its name, never twice, and gives letters no extra name", () => {
    onPlatform("MacIntel");
    const { container } = render(
      <>
        <Kbd>shift</Kbd>
        <Kbd>k</Kbd>
      </>,
    );
    const [shift, k] = [...container.querySelectorAll("kbd")];
    expect(shift).not.toHaveAttribute("title");
    expect(shift).not.toHaveAttribute("aria-label");
    expect(shift).toHaveTextContent("⇧Shift");
    expect(k?.textContent).toBe("K");
  });

  it("renders a repeated key in a sequence without a duplicate React key", () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    render(<KbdSequence keys={[["g", "g"]]} />);
    expect(error).not.toHaveBeenCalled();
    error.mockRestore();
  });
});
