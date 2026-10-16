import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Dialog, LayerDialog } from "./dialog";
import { StepDialog } from "./step-dialog";
import { Button } from "../button/button";
import { useState } from "react";

function OpenedFromOutside() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button onClick={() => setOpen(true)}>Rename key</Button>
      <Dialog open={open} onOpenChange={setOpen} title="Rename key">
        <input aria-label="Name" autoFocus />
      </Dialog>
    </>
  );
}

describe("Dialog", () => {
  it("opens from its trigger, closes with Escape and returns focus", async () => {
    render(
      <Dialog trigger={<Button>Create key</Button>} title="Create key" description="Applications use a key to call the gateway.">
        <p>Body</p>
      </Dialog>,
    );
    const trigger = screen.getByRole("button", { name: "Create key" });
    await userEvent.click(trigger);
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveAccessibleName("Create key");
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    await waitFor(() => expect(trigger).toHaveFocus());
  });
  it("opens on its first field, or on itself when it has none", async () => {
    const { unmount } = render(
      <Dialog defaultOpen title="Rename">
        <input aria-label="Name" />
      </Dialog>,
    );
    await waitFor(() => expect(screen.getByRole("textbox", { name: "Name" })).toHaveFocus());
    unmount();
    render(
      <Dialog defaultOpen title="Keyboard shortcuts">
        <p>Search and commands</p>
      </Dialog>,
    );
    await waitFor(() => expect(screen.getByRole("dialog")).toHaveFocus());
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "Close" })).toHaveFocus();
  });
  it("opened from a button outside it, gives the focus back to that button when it closes", async () => {
    render(<OpenedFromOutside />);
    const opener = screen.getByRole("button", { name: "Rename key" });
    await userEvent.click(opener);
    await screen.findByRole("dialog");
    await waitFor(() => expect(screen.getByRole("textbox", { name: "Name" })).toHaveFocus());
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    await waitFor(() => expect(opener).toHaveFocus());
  });
  it("⌘Enter submits", async () => {
    const onSubmit = vi.fn();
    render(
      <Dialog defaultOpen title="Rename" onSubmit={onSubmit}>
        <input aria-label="Name" />
      </Dialog>,
    );
    await screen.findByRole("dialog");
    await userEvent.keyboard("{Meta>}{Enter}{/Meta}");
    expect(onSubmit).toHaveBeenCalledOnce();
  });
});

describe("LayerDialog", () => {
  it("keeps one header height and goes back a layer", async () => {
    function Wizard() {
      return (
        <LayerDialog
          defaultOpen
          layers={[
            { title: "Add model", content: <p>Search</p> },
            { title: "Review values", content: <p>Prices</p> },
          ]}
          layer={1}
        />
      );
    }
    render(<Wizard />);
    await screen.findByRole("dialog");
    expect(screen.getByRole("button", { name: "Back" })).toBeInTheDocument();
    expect(screen.getByText("Review values")).toBeInTheDocument();
  });
});

describe("StepDialog", () => {
  it("goes one step at a time and submits on the last", async () => {
    const onSubmit = vi.fn();
    function Demo() {
      return (
        <StepDialog
          open
          title="Create key"
          submitLabel="Create key"
          onSubmit={onSubmit}
          steps={[
            { id: "name", title: "What is it for?", content: <input aria-label="Name" /> },
            { id: "expiry", title: "When does it expire?", content: <input aria-label="Days" /> },
          ]}
        />
      );
    }
    render(<Demo />);
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveAccessibleDescription("Step 1 of 2");
    expect(screen.getByRole("heading", { name: "What is it for?" })).toBeVisible();
    await userEvent.click(screen.getByRole("textbox", { name: "Name" }));
    await userEvent.keyboard("{Enter}");
    expect(dialog).toHaveAccessibleDescription("Step 2 of 2");
    await userEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(dialog).toHaveAccessibleDescription("Step 1 of 2");
    await userEvent.click(screen.getByRole("button", { name: "Continue" }));
    await userEvent.click(screen.getByRole("button", { name: "Create key" }));
    expect(onSubmit).toHaveBeenCalledOnce();
  });
  describe.each([
    ["⌘Enter", "{Meta>}{Enter}{/Meta}"],
    ["Ctrl+Enter", "{Control>}{Enter}{/Control}"],
  ])("from the keyboard alone, with %s", (_, submitKeys) => {
    function ThreeSteps({ onSubmit }: { onSubmit: () => void }) {
      return (
        <StepDialog
          open
          title="Create key"
          submitLabel="Create key"
          onSubmit={onSubmit}
          steps={[
            { id: "name", title: "What is it for?", content: <input aria-label="Name" /> },
            { id: "expiry", title: "When does it expire?", content: <input aria-label="Days" /> },
            { id: "review", title: "Ready to create it?", content: <p>The key is shown once, right after it is created.</p> },
          ]}
        />
      );
    }
    it("walks every step, the focus in the step it shows", async () => {
      const onSubmit = vi.fn();
      render(<ThreeSteps onSubmit={onSubmit} />);
      const dialog = await screen.findByRole("dialog");
      const live = dialog.querySelector("[aria-live=polite]");
      await waitFor(() => expect(screen.getByRole("textbox", { name: "Name" })).toHaveFocus());
      expect(live).toHaveTextContent(/^$/);

      await userEvent.keyboard("Batch exports");
      await userEvent.keyboard(submitKeys);
      expect(dialog).toHaveAccessibleDescription("Step 2 of 3");
      expect(screen.getByRole("textbox", { name: "Days" })).toHaveFocus();
      expect(live).toHaveTextContent("Step 2 of 3: When does it expire?");

      await userEvent.keyboard("30");
      await userEvent.keyboard(submitKeys);
      expect(dialog).toHaveAccessibleDescription("Step 3 of 3");
      const heading = screen.getByRole("heading", { name: "Ready to create it?" });
      expect(heading).toHaveFocus();
      expect(heading.closest("section")).toHaveAccessibleName("Ready to create it?");
      expect(live).toHaveTextContent("Step 3 of 3: Ready to create it?");

      await userEvent.keyboard(submitKeys);
      expect(onSubmit).toHaveBeenCalledOnce();
    });
    it("goes back with the focus in the previous step", async () => {
      render(<ThreeSteps onSubmit={() => undefined} />);
      const dialog = await screen.findByRole("dialog");
      await waitFor(() => expect(screen.getByRole("textbox", { name: "Name" })).toHaveFocus());
      await userEvent.keyboard("{Enter}");
      expect(screen.getByRole("textbox", { name: "Days" })).toHaveFocus();
      await userEvent.keyboard(submitKeys);
      expect(screen.getByRole("heading", { name: "Ready to create it?" })).toHaveFocus();
      await userEvent.tab();
      expect(screen.getByRole("button", { name: "Back" })).toHaveFocus();
      await userEvent.keyboard("{Enter}");
      expect(dialog).toHaveAccessibleDescription("Step 2 of 3");
      expect(screen.getByRole("textbox", { name: "Days" })).toHaveFocus();
      expect(dialog.querySelector("[aria-live=polite]")).toHaveTextContent("Step 2 of 3: When does it expire?");
      await userEvent.click(screen.getByRole("button", { name: "Back" }));
      expect(screen.getByRole("textbox", { name: "Name" })).toHaveFocus();
    });
  });
  it("won't continue past a blocked step", async () => {
    render(
      <StepDialog
        open
        title="Create key"
        submitLabel="Create key"
        onSubmit={() => undefined}
        steps={[
          { id: "a", title: "Name", blockedReason: "Enter a name", content: <input aria-label="Name" /> },
          { id: "b", title: "Expiry", content: null },
        ]}
      />,
    );
    await screen.findByRole("dialog");
    expect(screen.getByRole("button", { name: "Continue" })).toHaveAttribute("aria-disabled", "true");
  });
  describe("height", () => {
    const original = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetHeight");
    beforeEach(() => {
      Object.defineProperty(HTMLElement.prototype, "offsetHeight", {
        configurable: true,
        get(this: HTMLElement) {
          if (!this.parentElement?.hasAttribute("data-step-frame")) return 0;
          const shown = [...this.querySelectorAll("section")].find((s) => !s.hidden);
          return Number(shown?.querySelector<HTMLElement>("[data-h]")?.dataset.h ?? 0) + 16;
        },
      });
    });
    afterEach(() => {
      if (original) Object.defineProperty(HTMLElement.prototype, "offsetHeight", original);
      vi.unstubAllGlobals();
    });
    function Heights({ heights }: { heights: number[] }) {
      return (
        <StepDialog
          open
          title="Add limit"
          submitLabel="Add limit"
          onSubmit={() => undefined}
          steps={heights.map((h, i) => ({
            id: `s${i}`,
            title: `Step ${i + 1}`,
            content: (
              <div data-h={h}>
                <input aria-label={`Field ${i + 1}`} />
              </div>
            ),
          }))}
        />
      );
    }
    const frame = (dialog: HTMLElement) => dialog.querySelector<HTMLElement>("[data-step-frame]")!;
    const shownStep = (dialog: HTMLElement) => [...dialog.querySelectorAll("section")].find((s) => !s.hidden)!;

    it("takes each step's height, shorter and taller, and opens without motion", async () => {
      render(<Heights heights={[40, 300, 40]} />);
      const dialog = await screen.findByRole("dialog");
      expect(frame(dialog).style.height).toBe("56px");
      expect(shownStep(dialog)).not.toHaveAttribute("data-enter");
      await userEvent.click(screen.getByRole("button", { name: "Continue" }));
      expect(frame(dialog).style.height).toBe("316px");
      expect(shownStep(dialog)).toHaveAttribute("data-enter", "forward");
      expect(screen.getByRole("textbox", { name: "Field 2" })).toHaveFocus();
      await userEvent.click(screen.getByRole("button", { name: "Back" }));
      expect(frame(dialog).style.height).toBe("56px");
      expect(shownStep(dialog)).toHaveAttribute("data-enter", "back");
      expect(screen.getByRole("textbox", { name: "Field 1" })).toHaveFocus();
    });
    it("keeps one height for steps a few pixels apart, and lands on the 4px grid", async () => {
      render(<Heights heights={[100, 90, 103]} />);
      const dialog = await screen.findByRole("dialog");
      expect(frame(dialog).style.height).toBe("116px");
      await userEvent.click(screen.getByRole("button", { name: "Continue" }));
      expect(frame(dialog).style.height).toBe("116px");
      await userEvent.click(screen.getByRole("button", { name: "Continue" }));
      expect(frame(dialog).style.height).toBe("120px");
    });
    it("grows for content that grows within a step in the next frame, never inside the observer's callback", async () => {
      const callbacks: ResizeObserverCallback[] = [];
      vi.stubGlobal(
        "ResizeObserver",
        class {
          constructor(cb: ResizeObserverCallback) {
            callbacks.push(cb);
          }
          observe() {}
          unobserve() {}
          disconnect() {}
        },
      );
      render(<Heights heights={[40]} />);
      const dialog = await screen.findByRole("dialog");
      expect(frame(dialog).style.height).toBe("56px");
      dialog.querySelector<HTMLElement>("[data-h]")!.dataset.h = "80";
      for (const cb of callbacks) cb([], {} as ResizeObserver);
      expect(frame(dialog).style.height).toBe("56px");
      await waitFor(() => expect(frame(dialog).style.height).toBe("96px"));
    });
    it("with reduced motion, swaps the step without its drift", async () => {
      vi.stubGlobal("matchMedia", (q: string) => ({
        matches: q.includes("reduce"),
        media: q,
        addEventListener() {},
        removeEventListener() {},
        addListener() {},
        removeListener() {},
      }));
      render(<Heights heights={[40, 300]} />);
      const dialog = await screen.findByRole("dialog");
      await userEvent.click(screen.getByRole("button", { name: "Continue" }));
      expect(frame(dialog).style.height).toBe("316px");
      expect(shownStep(dialog)).not.toHaveAttribute("data-enter");
      expect(screen.getByRole("textbox", { name: "Field 2" })).toHaveFocus();
    });
  });
});
