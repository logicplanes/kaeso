import { describe, expect, it } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Tooltip } from "./tooltip.js";
import { TooltipProvider } from "./tooltip-provider.js";
import { IconButton } from "../icon-button/icon-button.js";
import { Button } from "../button/button.js";
import { icons } from "../../icons/vocabulary.js";
import { lazyModulesSettled } from "../../utils/lazy-module.js";

const settled = () => act(() => lazyModulesSettled());

describe("Tooltip for a screen reader", () => {
  it("describes its trigger when it says more than the trigger's name, and opens as a tooltip", async () => {
    render(
      <TooltipProvider>
        <Tooltip content="Requests over a hard limit are refused with 429." shape="multiline">
          <Button>Hard limit</Button>
        </Tooltip>
      </TooltipProvider>,
    );
    await settled();
    const trigger = screen.getByRole("button", { name: "Hard limit" });
    expect(trigger).toHaveAccessibleDescription("Requests over a hard limit are refused with 429.");
    await userEvent.tab();
    expect(trigger).toHaveFocus();
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Requests over a hard limit are refused with 429.");
  });

  it("adds no description when it repeats the name of an icon button", async () => {
    render(
      <TooltipProvider>
        <IconButton icon={icons.filter} aria-label="Filter" />
      </TooltipProvider>,
    );
    await settled();
    const trigger = screen.getByRole("button", { name: "Filter" });
    expect(trigger).not.toHaveAttribute("aria-describedby");
    expect(trigger).toHaveAccessibleDescription("");
  });

  it("gives a disabled button the reason it waits, and keeps the trigger's own description", async () => {
    render(
      <TooltipProvider>
        <p id="own">Keys are shown once.</p>
        <Button disabled disabledReason="Enter a name" aria-describedby="own">
          Continue
        </Button>
      </TooltipProvider>,
    );
    await settled();
    expect(screen.getByRole("button", { name: "Continue" })).toHaveAccessibleDescription("Keys are shown once. Enter a name");
  });

  it("describes nothing while it is disabled", async () => {
    render(
      <TooltipProvider>
        <Tooltip content="Requests over a hard limit are refused with 429." disabled>
          <Button>Hard limit</Button>
        </Tooltip>
      </TooltipProvider>,
    );
    await settled();
    expect(screen.getByRole("button", { name: "Hard limit" })).not.toHaveAttribute("aria-describedby");
  });
});
