import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Dialog, LayerDialog } from "./dialog.js";
import { StepDialog } from "./step-dialog.js";
import { Button } from "../button/button.js";
import { List } from "../list/list.js";
import { useState } from "react";

beforeEach(() => vi.useFakeTimers({ toFake: ["requestAnimationFrame", "cancelAnimationFrame"] }));
afterEach(() => vi.useRealTimers());

const frames = () => act(() => vi.runAllTimersAsync());

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
    await frames();
    const dialog = screen.getByRole("dialog");
    expect(dialog).toHaveAccessibleName("Create key");
    await userEvent.keyboard("{Escape}");
    await frames();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });
  it("opens on its first field", async () => {
    render(
      <Dialog defaultOpen title="Rename" footer={<Button variant="primary">Save</Button>}>
        <p>Applications use the name in logs.</p>
        <input aria-label="Name" />
      </Dialog>,
    );
    await frames();
    expect(screen.getByRole("textbox", { name: "Name" })).toHaveFocus();
  });
  it.each([
    [
      "on its primary action when it has no field",
      <Button key="d" variant="primary">
        Done
      </Button>,
      "Done",
    ],
    [
      "on the safe action when the primary one destroys",
      <Button key="d" variant="danger">
        Delete key
      </Button>,
      "Cancel",
    ],
    [
      "on the safe action when the primary one is unavailable",
      <Button key="d" variant="primary" disabled disabledReason="Copy the key first">
        Done
      </Button>,
      "Cancel",
    ],
  ])("opens %s, never on the dialog box", async (_, primary, name) => {
    render(
      <Dialog
        defaultOpen
        title="Key created"
        footer={
          <>
            <Button>Cancel</Button>
            {primary}
          </>
        }
      >
        <p>The key is shown once.</p>
        <Button>Copy key</Button>
      </Dialog>,
    );
    await frames();
    expect(screen.getByRole("button", { name })).toHaveFocus();
  });
  it("without a field or a footer, opens on its first control, and on Close when it has none", async () => {
    const { unmount } = render(
      <Dialog defaultOpen title="Key created">
        <p>The key is shown once.</p>
        <Button>Copy key</Button>
      </Dialog>,
    );
    await frames();
    expect(screen.getByRole("button", { name: "Copy key" })).toHaveFocus();
    unmount();
    render(
      <Dialog defaultOpen title="Keyboard shortcuts">
        <p>Search and commands</p>
      </Dialog>,
    );
    await frames();
    expect(screen.getByRole("button", { name: "Close" })).toHaveFocus();
    expect(screen.getByRole("dialog")).not.toHaveFocus();
  });
  it("prefers the control marked data-autofocus over the first field", async () => {
    render(
      <Dialog defaultOpen title="Rename">
        <input aria-label="Name" />
        <input aria-label="Description" data-autofocus />
      </Dialog>,
    );
    await frames();
    expect(screen.getByRole("textbox", { name: "Description" })).toHaveFocus();
  });
  it("opened from a button outside it, gives the focus back to that button when it closes", async () => {
    render(<OpenedFromOutside />);
    const opener = screen.getByRole("button", { name: "Rename key" });
    await userEvent.click(opener);
    await frames();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Name" })).toHaveFocus();
    await userEvent.keyboard("{Escape}");
    await frames();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(opener).toHaveFocus();
  });
  it("⌘Enter submits", async () => {
    const onSubmit = vi.fn();
    render(
      <Dialog defaultOpen title="Rename" onSubmit={onSubmit}>
        <input aria-label="Name" />
      </Dialog>,
    );
    await frames();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    await userEvent.keyboard("{Meta>}{Enter}{/Meta}");
    expect(onSubmit).toHaveBeenCalledOnce();
  });
});

type View = "search" | "review" | "custom" | "all";

function AddModels({ onSubmit, onOpenChange }: { onSubmit: (view: View) => void; onOpenChange?: (open: boolean) => void }) {
  const [view, setView] = useState<View>("search");
  const [model, setModel] = useState("");
  const search = {
    title: "Add models",
    content: (
      <>
        <input aria-label="Search models" data-autofocus />
        <List
          aria-label="Models on models.dev"
          items={[
            { id: "gpt-5-mini", title: "gpt-5-mini" },
            { id: "claude-haiku-4-5", title: "claude-haiku-4-5" },
          ]}
          onOpen={(item) => {
            setModel(item.id);
            setView("review");
          }}
        />
        <Button variant="link" onClick={() => setView("custom")}>
          Add a custom model
        </Button>
      </>
    ),
    footer: (
      <>
        <Button onClick={() => setView("all")}>Add every current model</Button>
        <Button variant="primary">Add models</Button>
      </>
    ),
  };
  const layers = {
    review: { title: `Review ${model}`, content: <input aria-label="Input price" />, footer: <Button variant="primary">Add model</Button> },
    custom: { title: "Add a custom model", content: <input aria-label="Model id" />, footer: <Button variant="primary">Continue</Button> },
    all: { title: "Add every current model", content: <p>Skips models already here.</p> },
  };
  return (
    <LayerDialog
      trigger={<Button>New models</Button>}
      onOpenChange={(o) => {
        if (!o) setView("search");
        onOpenChange?.(o);
      }}
      layer={view === "search" ? 0 : 1}
      onLayerChange={(l) => l === 0 && setView("search")}
      onSubmit={() => onSubmit(view)}
      layers={view === "search" ? [search] : [search, layers[view]]}
    />
  );
}

async function openAddModels(onSubmit: (view: View) => void = () => undefined) {
  render(<AddModels onSubmit={onSubmit} />);
  await userEvent.click(screen.getByRole("button", { name: "New models" }));
  await frames();
  expect(screen.getByRole("textbox", { name: "Search models" })).toHaveFocus();
}

const submit = () => userEvent.keyboard("{Meta>}{Enter}{/Meta}");

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
    await frames();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Back" })).toBeInTheDocument();
    expect(screen.getByText("Review values")).toBeInTheDocument();
  });
  it("opened from a List row with Enter, moves the focus into the new layer, and Back returns it to the row", async () => {
    const onSubmit = vi.fn();
    await openAddModels(onSubmit);
    await userEvent.tab();
    await userEvent.keyboard("{ArrowDown}");
    const row = screen.getByRole("gridcell", { name: /claude-haiku-4-5/ });
    expect(row).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    expect(screen.getByRole("dialog")).toHaveAccessibleName("Review claude-haiku-4-5");
    expect(screen.getByRole("textbox", { name: "Input price" })).toHaveFocus();
    await submit();
    expect(onSubmit).toHaveBeenLastCalledWith("review");
    act(() => screen.getByRole("button", { name: "Back" }).focus());
    await userEvent.keyboard("{Enter}");
    expect(screen.getByRole("dialog")).toHaveAccessibleName("Add models");
    expect(screen.queryByRole("button", { name: "Back" })).toBeNull();
    expect(row).toHaveFocus();
    await submit();
    expect(onSubmit).toHaveBeenLastCalledWith("search");
    expect(onSubmit).toHaveBeenCalledTimes(2);
  });
  it("opened from a button, lands on the new layer's first field, and Back returns to the button", async () => {
    const onSubmit = vi.fn();
    await openAddModels(onSubmit);
    const opener = screen.getByRole("button", { name: "Add a custom model" });
    await userEvent.click(opener);
    expect(screen.getByRole("textbox", { name: "Model id" })).toHaveFocus();
    await submit();
    expect(onSubmit).toHaveBeenLastCalledWith("custom");
    await userEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(opener).toHaveFocus();
    await submit();
    expect(onSubmit).toHaveBeenLastCalledWith("search");
  });
  it("on a layer without a field, lands on its primary action, and Back returns to the footer button that opened it", async () => {
    const onSubmit = vi.fn();
    function Review() {
      const [layer, setLayer] = useState(0);
      return (
        <LayerDialog
          defaultOpen
          layer={layer}
          onLayerChange={setLayer}
          onSubmit={() => onSubmit(layer)}
          layers={[
            { title: "Add models", content: <input aria-label="Search models" />, footer: <Button onClick={() => setLayer(1)}>Review values</Button> },
            { title: "Review values", content: <p>Prices come from models.dev.</p>, footer: <Button variant="primary">Add model</Button> },
          ]}
        />
      );
    }
    render(<Review />);
    await frames();
    const opener = screen.getByRole("button", { name: "Review values" });
    act(() => opener.focus());
    await userEvent.keyboard("{Enter}");
    expect(screen.getByRole("button", { name: "Add model" })).toHaveFocus();
    await submit();
    expect(onSubmit).toHaveBeenLastCalledWith(1);
    await userEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(opener).toHaveFocus();
    await submit();
    expect(onSubmit).toHaveBeenLastCalledWith(0);
  });
  it("returns to the first control of the layer it shows when the opener is gone", async () => {
    const onSubmit = vi.fn();
    await openAddModels(onSubmit);
    await userEvent.click(screen.getByRole("button", { name: "Add every current model" }));
    expect(screen.getByRole("dialog")).toHaveAccessibleName("Add every current model");
    expect(screen.getByRole("button", { name: "Back" })).toHaveFocus();
    await submit();
    expect(onSubmit).toHaveBeenLastCalledWith("all");
    await userEvent.keyboard("{Enter}");
    expect(screen.getByRole("textbox", { name: "Search models" })).toHaveFocus();
    await submit();
    expect(onSubmit).toHaveBeenLastCalledWith("search");
  });
  it("starts a new layer at the top and gives the layer below its scroll position back", async () => {
    await openAddModels();
    const body = screen.getByRole("dialog").querySelector<HTMLElement>("[data-dialog-body]")!;
    body.scrollTop = 120;
    await userEvent.click(screen.getByRole("button", { name: "Add a custom model" }));
    expect(body.scrollTop).toBe(0);
    await userEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(body.scrollTop).toBe(120);
  });
  it("closed from a layer, leaves the layers below alone and gives the focus back to its trigger", async () => {
    const onOpenChange = vi.fn();
    render(<AddModels onSubmit={() => undefined} onOpenChange={onOpenChange} />);
    const trigger = screen.getByRole("button", { name: "New models" });
    await userEvent.click(trigger);
    await frames();
    const opener = screen.getByRole("button", { name: "Add a custom model" });
    await userEvent.click(opener);
    expect(screen.getByRole("textbox", { name: "Model id" })).toHaveFocus();
    const focusedBelow = vi.fn();
    opener.addEventListener("focus", focusedBelow);
    await userEvent.keyboard("{Escape}");
    await frames();
    expect(focusedBelow).not.toHaveBeenCalled();
    expect(onOpenChange).toHaveBeenLastCalledWith(false);
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(trigger).toHaveFocus();
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
    await frames();
    const dialog = screen.getByRole("dialog");
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
      await frames();
      const dialog = screen.getByRole("dialog");
      const live = dialog.querySelector("[aria-live=polite]");
      expect(screen.getByRole("textbox", { name: "Name" })).toHaveFocus();
      expect(live).toHaveTextContent(/^$/);

      await userEvent.keyboard("Batch exports");
      await userEvent.keyboard(submitKeys);
      expect(dialog).toHaveAccessibleDescription("Step 2 of 3");
      expect(screen.getByRole("textbox", { name: "Days" })).toHaveFocus();
      expect(live).toHaveTextContent("Step 2 of 3: When does it expire?");

      await userEvent.keyboard("30");
      await userEvent.keyboard(submitKeys);
      expect(dialog).toHaveAccessibleDescription("Step 3 of 3");
      expect(screen.getByRole("button", { name: "Create key" })).toHaveFocus();
      expect(screen.getByRole("heading", { name: "Ready to create it?" }).closest("section")).toHaveAccessibleName("Ready to create it?");
      expect(live).toHaveTextContent("Step 3 of 3: Ready to create it?");

      await userEvent.keyboard(submitKeys);
      expect(onSubmit).toHaveBeenCalledOnce();
    });
    it("goes back with the focus in the previous step", async () => {
      render(<ThreeSteps onSubmit={() => undefined} />);
      await frames();
      const dialog = screen.getByRole("dialog");
      expect(screen.getByRole("textbox", { name: "Name" })).toHaveFocus();
      await userEvent.keyboard("{Enter}");
      expect(screen.getByRole("textbox", { name: "Days" })).toHaveFocus();
      await userEvent.keyboard(submitKeys);
      expect(screen.getByRole("button", { name: "Create key" })).toHaveFocus();
      await userEvent.tab({ shift: true });
      expect(screen.getByRole("button", { name: "Back" })).toHaveFocus();
      await userEvent.keyboard("{Enter}");
      expect(dialog).toHaveAccessibleDescription("Step 2 of 3");
      expect(screen.getByRole("textbox", { name: "Days" })).toHaveFocus();
      expect(dialog.querySelector("[aria-live=polite]")).toHaveTextContent("Step 2 of 3: When does it expire?");
      await userEvent.click(screen.getByRole("button", { name: "Back" }));
      expect(screen.getByRole("textbox", { name: "Name" })).toHaveFocus();
    });
  });
  it("lands on Back, not the destructive submit, on a last step without a field", async () => {
    render(
      <StepDialog
        open
        tone="danger"
        title="Delete project"
        submitLabel="Delete project"
        onSubmit={() => undefined}
        steps={[
          { id: "name", title: "Which project?", content: <input aria-label="Project" /> },
          { id: "confirm", title: "Delete it?", content: <p>Its keys stop working at once.</p> },
        ]}
      />,
    );
    await frames();
    await userEvent.keyboard("{Enter}");
    expect(screen.getByRole("button", { name: "Back" })).toHaveFocus();
  });
  it("opens on its first step's field, not on a field of a later step", async () => {
    render(
      <StepDialog
        open
        title="Key created"
        submitLabel="Done"
        onSubmit={() => undefined}
        defaultStep={1}
        steps={[
          { id: "name", title: "Name", content: <input aria-label="Name" /> },
          { id: "done", title: "Copy the key", content: <p>It is shown once.</p> },
        ]}
      />,
    );
    await frames();
    expect(screen.getByRole("button", { name: "Done" })).toHaveFocus();
  });
  it("names each step by its own heading when two step dialogs share step ids", async () => {
    const steps = (name: string) => [{ id: "name", title: name, content: <input aria-label={name} /> }];
    render(
      <>
        <StepDialog inline title="Create key" submitLabel="Create key" onSubmit={() => undefined} steps={steps("Key name")} />
        <StepDialog inline title="Create project" submitLabel="Create project" onSubmit={() => undefined} steps={steps("Project name")} />
      </>,
    );
    await frames();
    expect(screen.getByRole("region", { name: "Key name" })).toContainElement(screen.getByRole("textbox", { name: "Key name" }));
    expect(screen.getByRole("region", { name: "Project name" })).toContainElement(screen.getByRole("textbox", { name: "Project name" }));
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
    await frames();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
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
      await frames();
      const dialog = screen.getByRole("dialog");
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
      await frames();
      const dialog = screen.getByRole("dialog");
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
      await frames();
      const dialog = screen.getByRole("dialog");
      expect(frame(dialog).style.height).toBe("56px");
      dialog.querySelector<HTMLElement>("[data-h]")!.dataset.h = "80";
      for (const cb of callbacks) cb([], {} as ResizeObserver);
      expect(frame(dialog).style.height).toBe("56px");
      await act(() => vi.advanceTimersToNextFrame());
      expect(frame(dialog).style.height).toBe("96px");
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
      await frames();
      const dialog = screen.getByRole("dialog");
      await userEvent.click(screen.getByRole("button", { name: "Continue" }));
      expect(frame(dialog).style.height).toBe("316px");
      expect(shownStep(dialog)).not.toHaveAttribute("data-enter");
      expect(screen.getByRole("textbox", { name: "Field 2" })).toHaveFocus();
    });
  });
});
