import { describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Tooltip } from "./tooltip.js";
import { TooltipProvider } from "./tooltip-provider.js";
import { hoverWithMouse } from "../../../test/hover.js";
import { lazyModulesSettled } from "../../utils/lazy-module.js";

const loads = vi.hoisted(() => ({ attempts: 0, failing: 1 }));
vi.mock("./tooltip-root.js", async (importOriginal) => {
  loads.attempts++;
  if (loads.failing-- > 0) throw new TypeError("Importing a module script failed.");
  return importOriginal();
});

await vi.importActual("./tooltip-root.js");

describe("a tooltip whose code failed to load", () => {
  it("keeps the trigger working, loads the code again when the pointer or the focus comes back, and then shows", async () => {
    const user = userEvent.setup();
    const onPointerEnter = vi.fn();
    const onFocus = vi.fn();
    render(
      <TooltipProvider>
        <Tooltip content="Rotate the key">
          <button type="button" aria-label="Rotate" onPointerEnter={onPointerEnter} onFocus={onFocus} />
        </Tooltip>
        <button type="button">Elsewhere</button>
      </TooltipProvider>,
    );
    await act(() => lazyModulesSettled());
    expect(loads.attempts).toBe(1);
    await act(() => lazyModulesSettled());
    expect(loads.attempts).toBe(1);

    const trigger = screen.getByRole("button", { name: "Rotate" });
    hoverWithMouse(trigger);
    expect(onPointerEnter).toHaveBeenCalledTimes(1);
    await act(() => lazyModulesSettled());
    expect(loads.attempts).toBe(2);
    expect(trigger).toHaveAttribute("data-base-ui-tooltip-trigger");

    await user.click(screen.getByRole("button", { name: "Elsewhere" }));
    await user.tab({ shift: true });
    expect(trigger).toHaveFocus();
    expect(onFocus).toHaveBeenCalled();
    const tip = [...document.querySelectorAll("[data-side]")].find((el) => el.textContent?.startsWith("Rotate the key"));
    expect(tip).toHaveAttribute("data-open");
  });
});
