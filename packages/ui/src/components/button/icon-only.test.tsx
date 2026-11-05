import { describe, expect, it } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Button } from "./button.js";
import { TooltipProvider } from "../tooltip/tooltip-provider.js";
import { icons } from "../../icons/vocabulary.js";
import { lazyModulesSettled } from "../../utils/lazy-module.js";

const settled = () => act(() => lazyModulesSettled());

describe("an icon-only button", () => {
  it("is a circle named by its aria-label, which its tooltip shows on hover", async () => {
    render(
      <TooltipProvider delay={0}>
        <Button variant="ghost" size="sm" icon={icons.filter} aria-label="Filter" />
      </TooltipProvider>,
    );
    await settled();
    const button = screen.getByRole("button", { name: "Filter" });
    expect(button).toHaveClass("rounded-pill", "size-32");
    await userEvent.hover(button);
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Filter");
  });

  it("shows a shortcut in its tooltip and announces it as a key shortcut", async () => {
    render(
      <TooltipProvider delay={0}>
        <Button size="sm" icon={icons.search} aria-label="Search" shortcut={["mod", "k"]} />
      </TooltipProvider>,
    );
    await settled();
    const button = screen.getByRole("button", { name: "Search" });
    expect(button).toHaveAttribute("aria-keyshortcuts", "Meta+K Control+K");
    await userEvent.hover(button);
    expect((await screen.findByRole("tooltip")).querySelector("kbd")).not.toBeNull();
  });

  it("has no tooltip when it is turned off or the button is disabled without a reason", async () => {
    render(
      <TooltipProvider delay={0}>
        <Button icon={icons.copy} aria-label="Copy key" tooltip={false} />
        <Button icon={icons.add} aria-label="Add key" disabled />
      </TooltipProvider>,
    );
    await settled();
    await userEvent.hover(screen.getByRole("button", { name: "Copy key" }));
    await settled();
    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  it("keeps focus when it becomes disabled with a reason while focused", () => {
    const { rerender } = render(<Button variant="ghost" size="sm" icon={icons.add} aria-label="Add column" />);
    const add = screen.getByRole("button", { name: "Add column" });
    add.focus();
    rerender(<Button variant="ghost" size="sm" icon={icons.add} aria-label="Add column" disabled disabledReason="A table holds at most 12 columns" />);
    expect(document.activeElement).toBe(add);
    expect(add).toHaveAttribute("aria-description", "A table holds at most 12 columns");
  });
});
