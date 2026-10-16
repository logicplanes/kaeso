import { describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRef } from "react";
import { Tooltip } from "./tooltip";
import { TooltipProvider } from "./tooltip-provider";

const gate = vi.hoisted(() => {
  let open!: () => void;
  const promise = new Promise<void>((resolve) => (open = resolve));
  return { promise, open };
});
vi.mock("./tooltip-root", async (importOriginal) => {
  await gate.promise;
  return importOriginal();
});

async function arrive() {
  gate.open();
  await act(async () => {
    await import("./tooltip-root");
    await new Promise((resolve) => setTimeout(resolve));
  });
}

describe("a tooltip's trigger while the tooltip loads", () => {
  it("is the same element before and after: the focus and a press in flight stay, and the click lands once", async () => {
    const user = userEvent.setup();
    const onClick = vi.fn();
    render(
      <TooltipProvider>
        <Tooltip content="Copy the key">
          <button type="button" aria-label="Copy" onClick={onClick} />
        </Tooltip>
      </TooltipProvider>,
    );
    const trigger = screen.getByRole("button", { name: "Copy" });
    await user.tab();
    expect(trigger).toHaveFocus();
    await user.pointer({ keys: "[MouseLeft>]", target: trigger });

    await arrive();
    expect(screen.getByRole("button", { name: "Copy" })).toBe(trigger);
    expect(trigger.isConnected).toBe(true);
    expect(trigger).toHaveFocus();

    await user.pointer({ keys: "[/MouseLeft]", target: screen.getByRole("button", { name: "Copy" }) });
    expect(onClick).toHaveBeenCalledTimes(1);
    expect(trigger).toHaveFocus();
  }, 20_000);

  it("once loaded, opens on hover after the delay and on keyboard focus, and keeps the element's own ref and props", async () => {
    const user = userEvent.setup();
    const ref = createRef<HTMLButtonElement>();
    const onClick = vi.fn();
    render(
      <TooltipProvider>
        <Tooltip content="Rotate the key" shortcut={["mod", "r"]}>
          <button ref={ref} type="button" aria-label="Rotate" className="own" onClick={onClick} />
        </Tooltip>
        <button type="button">Elsewhere</button>
      </TooltipProvider>,
    );
    const trigger = screen.getByRole("button", { name: "Rotate" });
    expect(ref.current).toBe(trigger);
    expect(trigger).toHaveClass("own");
    expect(trigger).toHaveAttribute("data-base-ui-tooltip-trigger");
    const tip = () => [...document.querySelectorAll("[data-side]")].find((el) => el.textContent?.startsWith("Rotate the key"));

    await user.hover(trigger);
    expect(tip()).toBeUndefined();
    await waitFor(() => expect(tip()).toHaveAttribute("data-open"), { timeout: 2_000 });
    expect(trigger).toHaveAttribute("data-popup-open");
    await user.click(trigger);
    expect(onClick).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(trigger).not.toHaveAttribute("data-popup-open"));

    await user.unhover(trigger);
    await user.click(screen.getByRole("button", { name: "Elsewhere" }));
    await user.tab({ shift: true });
    expect(trigger).toHaveFocus();
    await waitFor(() => expect(tip()).toHaveAttribute("data-open"), { timeout: 2_000 });
    expect(screen.getByRole("button", { name: "Rotate" })).toBe(trigger);
  }, 20_000);
});
