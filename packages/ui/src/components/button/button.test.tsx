import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Button } from "./button.js";
import { SwapText } from "../swap-text/swap-text.js";
import { icons } from "../../icons/vocabulary.js";

describe("Button", () => {
  it("keeps focus when it becomes disabled while focused, and disables natively once focus leaves", async () => {
    const onClick = vi.fn();
    const { rerender } = render(
      <>
        <Button onClick={onClick}>Stop</Button>
        <Button>Next</Button>
      </>,
    );
    const stop = screen.getByRole("button", { name: "Stop" });
    stop.focus();
    rerender(
      <>
        <Button onClick={onClick} disabled>
          Stop
        </Button>
        <Button>Next</Button>
      </>,
    );
    expect(document.activeElement).toBe(stop);
    expect(stop).toHaveAttribute("aria-disabled", "true");
    expect(stop).not.toBeDisabled();
    await userEvent.keyboard("{Enter}");
    expect(onClick).not.toHaveBeenCalled();
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "Stop" })).toBeDisabled();
  });
  it("is a pill button that calls onClick", async () => {
    const onClick = vi.fn();
    render(<Button onClick={onClick}>Create key</Button>);
    await userEvent.click(screen.getByRole("button", { name: "Create key" }));
    expect(onClick).toHaveBeenCalledOnce();
    expect(screen.getByRole("button").className).toContain("rounded-pill");
  });
  it("loading keeps the button enabled-looking but inert, and marks it busy", async () => {
    const onClick = vi.fn();
    render(
      <Button loading onClick={onClick}>
        Save
      </Button>,
    );
    const b = screen.getByRole("button");
    expect(b).toHaveAttribute("data-loading");
    expect(b).toHaveAttribute("aria-busy", "true");
    await userEvent.click(b);
    expect(onClick).not.toHaveBeenCalled();
  });
  it("pulls a ghost button's padding past a settings row's edge, so its label lines up with outlined buttons", () => {
    render(
      <>
        <Button variant="ghost" size="sm">
          End session
        </Button>
        <Button variant="quiet" size="md">
          Show
        </Button>
        <Button variant="ghost" shape="circle" size="sm" icon={icons.more} aria-label="More" />
      </>,
    );
    expect(screen.getByRole("button", { name: "End session" })).toHaveClass("material-ghost", "px-12", "ghost-end:-mr-12", "ghost-start:-ml-12");
    expect(screen.getByRole("button", { name: "Show" })).toHaveClass("material-ghost", "px-16", "ghost-end:-mr-16", "ghost-start:-ml-16");
    const circle = screen.getByRole("button", { name: "More" });
    expect(circle).toHaveClass("ghost-end:mr-0", "ghost-start:ml-0");
    expect(circle).not.toHaveClass("ghost-end:-mr-12");
  });
  it("reserves the width of every label it can show", () => {
    const { container } = render(<Button reserveLabels={["Copied"]}>Copy page</Button>);
    expect(container.textContent).toContain("Copied");
  });
});

describe("SwapText", () => {
  it("renders the visible label once for assistive technology and reserves the others hidden", () => {
    render(<SwapText reserve={["Copied"]}>Copy</SwapText>);
    expect(screen.getByText("Copy")).toBeVisible();
    expect(screen.getByText("Copied")).toHaveAttribute("aria-hidden", "true");
  });
});
