import { describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor } from "@testing-library/react";
import { lazyModulesSettled } from "../../utils/lazy-module.js";
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
  it("keeps focus when it becomes disabled with a reason while focused", () => {
    const { rerender } = render(<Button>Add column</Button>);
    const add = screen.getByRole("button", { name: "Add column" });
    add.focus();
    rerender(
      <Button disabled disabledReason="A table holds at most 12 columns">
        Add column
      </Button>,
    );
    expect(document.activeElement).toBe(add);
    expect(screen.getByRole("button", { name: "Add column" })).toBe(add);
    expect(add).toHaveAttribute("aria-description", "A table holds at most 12 columns");
    rerender(<Button>Add column</Button>);
    expect(document.activeElement).toBe(add);
  });

  it("keeps focus when its label starts and stops being cut off", async () => {
    const observers: (() => void)[] = [];
    vi.stubGlobal(
      "ResizeObserver",
      class {
        constructor(private readonly run: () => void) {}
        observe() {
          observers.push(this.run);
        }
        unobserve() {}
        disconnect() {}
      },
    );
    vi.stubGlobal("requestAnimationFrame", (run: FrameRequestCallback) => {
      run(0);
      return 0;
    });
    let scrollWidth = 120;
    const width = vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockImplementation(() => scrollWidth);
    const client = vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(120);
    render(<Button>Read the gateway setup guide</Button>);
    const button = screen.getByRole("button", { name: "Read the gateway setup guide" });
    button.focus();
    const resize = (next: number) => {
      scrollWidth = next;
      act(() => observers.forEach((run) => run()));
    };
    resize(240);
    await act(() => lazyModulesSettled());
    expect(document.activeElement).toBe(button);
    expect(button).toHaveAttribute("data-base-ui-tooltip-trigger");
    resize(120);
    expect(document.activeElement).toBe(button);
    width.mockRestore();
    client.mockRestore();
    vi.unstubAllGlobals();
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

  it("reads out why a soft-disabled button is unavailable", () => {
    render(
      <Button disabled disabledReason="Ask an owner to add models">
        Add model
      </Button>,
    );
    const button = screen.getByRole("button", { name: "Add model" });
    expect(button).toHaveAttribute("aria-disabled", "true");
    expect(button).toHaveAttribute("aria-description", "Ask an owner to add models");
  });

  it("keeps the shortcut out of the name, so the name is the visible label, and announces it as its key shortcut", () => {
    render(
      <>
        <Button shortcut={["mod", "k"]}>Search</Button>
        <Button shortcut={["shift", "enter"]}>Send</Button>
        <Button shortcut={[["g"], ["l"]]}>Logs</Button>
      </>,
    );
    expect(screen.getByRole("button", { name: "Search" })).toHaveAttribute("aria-keyshortcuts", "Meta+K Control+K");
    expect(screen.getByRole("button", { name: "Send" })).toHaveAttribute("aria-keyshortcuts", "Shift+Enter");
    expect(screen.getByRole("button", { name: "Logs" })).not.toHaveAttribute("aria-keyshortcuts");
  });

  it("shows a cut label whole in a tooltip", async () => {
    const width = vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockReturnValue(240);
    const client = vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(120);
    render(<Button>Read the gateway setup guide</Button>);
    await act(() => lazyModulesSettled());
    const trigger = screen.getByRole("button", { name: "Read the gateway setup guide" });
    expect(trigger).toHaveAttribute("data-base-ui-tooltip-trigger");
    await userEvent.tab();
    expect(trigger).toHaveFocus();
    await waitFor(() => expect([...document.querySelectorAll("[data-side]")].some((el) => el.textContent === "Read the gateway setup guide")).toBe(true));
    width.mockRestore();
    client.mockRestore();
  });
});
