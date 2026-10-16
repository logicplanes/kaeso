import { afterEach, describe, expect, it, vi } from "vitest";
import { act, screen } from "@testing-library/react";
import { renderToString } from "react-dom/server";
import { hydrateRoot } from "react-dom/client";
import { Kbd } from "./kbd.js";

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
    expect(screen.getByText("⌘")).toHaveAccessibleName("Command");
    act(() => root.unmount());
  });
});
