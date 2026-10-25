import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AlertDialog, Dialog, LayerDialog } from "./dialog.js";
import { StepDialog } from "./step-dialog.js";
import { Button } from "../button/button.js";
import { List } from "../list/list.js";
import { TagInput } from "../input/tag-input.js";
import { Combobox } from "../combobox/combobox.js";
import { createRef, useState } from "react";
import { readFileSync } from "node:fs";
import { join } from "node:path";

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
  it("closing after another dialog opened leaves the focus and the search in the new one", async () => {
    function RenameThenAddOwner() {
      const [renaming, setRenaming] = useState(false);
      const [adding, setAdding] = useState(false);
      return (
        <>
          <Button onClick={() => setRenaming(true)}>Rename</Button>
          <Button onClick={() => setAdding(true)}>Add owner</Button>
          <Dialog open={renaming} onOpenChange={setRenaming} title="Rename the organization">
            <Button onClick={() => setRenaming(false)}>Save name</Button>
          </Dialog>
          <Dialog open={adding} onOpenChange={setAdding} title="Add an owner">
            <Combobox label="User" options={[{ value: "usr_lin", label: "Lin Park" }]} />
          </Dialog>
        </>
      );
    }
    let finishClosing = () => undefined as void;
    const closing = { finished: new Promise<void>((resolve) => (finishClosing = resolve)) } as unknown as Animation;
    const animations = vi.spyOn(Element.prototype, "getAnimations").mockImplementation(function (this: Element) {
      return this.closest("[data-ending-style]") ? [closing] : [];
    });
    render(<RenameThenAddOwner />);
    await userEvent.click(screen.getByRole("button", { name: "Rename" }));
    await frames();
    await userEvent.click(screen.getByRole("button", { name: "Save name" }));
    await userEvent.click(screen.getByRole("button", { name: "Add owner" }));
    await frames();
    const search = screen.getByRole("combobox", { name: "User" });
    await userEvent.type(search, "Lin");
    expect(screen.getByRole("dialog", { name: "Rename the organization", hidden: true })).toBeInTheDocument();
    finishClosing();
    await frames();
    animations.mockRestore();
    expect(screen.queryByRole("dialog", { name: "Rename the organization" })).not.toBeInTheDocument();
    expect(search).toHaveFocus();
    expect(search).toHaveValue("Lin");
    expect(screen.getByRole("option", { name: "Lin Park" })).toBeVisible();
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
  it("Enter in a single-line field submits, like a form", async () => {
    const onSubmit = vi.fn();
    render(
      <Dialog defaultOpen title="Sign in again to continue" onSubmit={onSubmit}>
        <input aria-label="Password" type="password" />
      </Dialog>,
    );
    await frames();
    await userEvent.type(screen.getByLabelText("Password"), "correct horse{Enter}");
    expect(onSubmit).toHaveBeenCalledOnce();
  });
  it("Enter keeps its own meaning in a text area, a combobox, a search field, on a button and while an input method composes", async () => {
    const onSubmit = vi.fn();
    render(
      <Dialog defaultOpen title="Edit prompt" onSubmit={onSubmit}>
        <textarea aria-label="Prompt" />
        <input aria-label="Model" role="combobox" />
        <input aria-label="Name" />
        <input aria-label="Search models" type="search" />
        <input aria-label="Tag" onKeyDown={(e) => e.key === "Enter" && e.preventDefault()} />
        <input aria-label="Notify" type="checkbox" />
        <button type="button">Preview</button>
      </Dialog>,
    );
    await frames();
    await userEvent.type(screen.getByLabelText("Prompt"), "one{Enter}two");
    expect(screen.getByLabelText("Prompt")).toHaveValue("one\ntwo");
    await userEvent.type(screen.getByRole("combobox", { name: "Model" }), "{Enter}");
    await userEvent.type(screen.getByLabelText("Tag"), "eu{Enter}");
    screen.getByLabelText("Notify").focus();
    await userEvent.keyboard("{Enter}");
    screen.getByRole("button", { name: "Preview" }).focus();
    await userEvent.keyboard("{Enter}");
    await userEvent.type(screen.getByRole("searchbox", { name: "Search models" }), "gpt{Enter}");
    const field = screen.getByRole("textbox", { name: "Name" });
    fireEvent.keyDown(field, { key: "Enter", isComposing: true });
    fireEvent.keyDown(field, { key: "Enter", keyCode: 229 });
    expect(onSubmit).not.toHaveBeenCalled();
    fireEvent.keyDown(field, { key: "Enter", keyCode: 13 });
    expect(onSubmit).toHaveBeenCalledOnce();
    onSubmit.mockClear();
    await userEvent.type(screen.getByLabelText("Prompt"), "{Control>}{Enter}{/Control}");
    expect(onSubmit).toHaveBeenCalledOnce();
  });
  it("⌘Enter submits from a control that handles Enter itself", async () => {
    const onSubmit = vi.fn();
    render(
      <Dialog defaultOpen title="Edit limit" onSubmit={onSubmit}>
        <button type="button" role="switch" aria-checked="false" onKeyDown={(e) => e.key === "Enter" && e.preventDefault()}>
          Hard limit
        </button>
      </Dialog>,
    );
    await frames();
    screen.getByRole("switch").focus();
    await userEvent.keyboard("{Enter}");
    expect(onSubmit).not.toHaveBeenCalled();
    await userEvent.keyboard("{Meta>}{Enter}{/Meta}");
    expect(onSubmit).toHaveBeenCalledOnce();
  });
  it("Enter does nothing while the dialog is busy", async () => {
    const onSubmit = vi.fn();
    render(
      <Dialog defaultOpen busy title="Rename project" onSubmit={onSubmit}>
        <input aria-label="Name" />
      </Dialog>,
    );
    await frames();
    await userEvent.type(screen.getByLabelText("Name"), "Atlas{Enter}");
    expect(onSubmit).not.toHaveBeenCalled();
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

function TwoFactor() {
  const [codes, setCodes] = useState(false);
  return (
    <Dialog open title={codes ? "Store your recovery codes" : "Set up two-factor authentication"}>
      {codes ? (
        <input type="checkbox" aria-label="I stored these codes" />
      ) : (
        <button type="button" onClick={() => setCodes(true)}>
          Turn on
        </button>
      )}
    </Dialog>
  );
}

describe("Dialog content that changes", () => {
  it("keeps the same dialog and moves the focus into the new content when the focused control goes away", async () => {
    render(<TwoFactor />);
    await frames();
    const dialog = screen.getByRole("dialog");
    await userEvent.click(screen.getByRole("button", { name: "Turn on" }));
    await frames();
    expect(screen.getByRole("dialog")).toBe(dialog);
    expect(dialog).toHaveAccessibleName("Store your recovery codes");
    expect(screen.getByRole("checkbox", { name: "I stored these codes" })).toHaveFocus();
  });
  it("leaves the focus where it is while it stays in the dialog", async () => {
    function Growing() {
      const [more, setMore] = useState(false);
      return (
        <Dialog open title="Invite member">
          <button type="button" onClick={() => setMore(true)}>
            Add projects
          </button>
          {more && <input aria-label="Project" />}
        </Dialog>
      );
    }
    render(<Growing />);
    await frames();
    await userEvent.click(screen.getByRole("button", { name: "Add projects" }));
    expect(screen.getByRole("button", { name: "Add projects" })).toHaveFocus();
  });
});

describe("busy dialogs", () => {
  it("stay open on a press outside when persistent, as every step dialog does, and still close with Escape", async () => {
    const onOpenChange = vi.fn();
    const { rerender } = render(<Dialog open persistent onOpenChange={onOpenChange} title="Create key" />);
    await frames();
    await userEvent.click(document.querySelector<HTMLElement>(".bg-overlay")!);
    expect(onOpenChange).not.toHaveBeenCalled();
    await userEvent.keyboard("{Escape}");
    expect(onOpenChange).toHaveBeenCalledWith(false);
    onOpenChange.mockClear();
    rerender(
      <StepDialog
        open
        onOpenChange={onOpenChange}
        title="Create key"
        submitLabel="Create key"
        onSubmit={() => undefined}
        steps={[
          { id: "name", title: "What is it for?", content: <input aria-label="Name" /> },
          { id: "expiry", title: "When does it expire?", content: null },
        ]}
      />,
    );
    await frames();
    await userEvent.click(document.querySelector<HTMLElement>(".bg-overlay")!);
    expect(onOpenChange).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Continue" }).textContent).toBe("Continue");
  });

  it("stay open while their work is underway: Escape, a press outside, the close circle, Cancel and ⌘Enter do nothing", async () => {
    const onOpenChange = vi.fn();
    const onSubmit = vi.fn();
    function Saving() {
      const [busy, setBusy] = useState(true);
      return (
        <Dialog
          open
          busy={busy}
          onOpenChange={onOpenChange}
          onSubmit={onSubmit}
          title="Rename key"
          footer={
            <>
              <Dialog.Close>
                <Button>Cancel</Button>
              </Dialog.Close>
              <Button onClick={() => setBusy(false)}>Saved</Button>
            </>
          }
        >
          <input aria-label="Name" />
        </Dialog>
      );
    }
    render(<Saving />);
    await frames();
    expect(screen.getByRole("button", { name: "Close" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeDisabled();
    await userEvent.keyboard("{Escape}");
    await userEvent.click(document.querySelector<HTMLElement>(".bg-overlay")!);
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    screen.getByRole("textbox", { name: "Name" }).focus();
    await userEvent.keyboard("{Control>}{Enter}{/Control}{Meta>}{Enter}{/Meta}");
    await frames();
    expect(onOpenChange).not.toHaveBeenCalled();
    expect(onSubmit).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Saved" }));
    expect(screen.getByRole("button", { name: "Close" })).toBeEnabled();
    screen.getByRole("textbox", { name: "Name" }).focus();
    await userEvent.keyboard("{Control>}{Enter}{/Control}");
    expect(onSubmit).toHaveBeenCalledOnce();
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });
  it("include a layer dialog, whose Back waits so no other layer can start a second request", async () => {
    const onLayerChange = vi.fn();
    render(
      <LayerDialog
        open
        busy
        layer={1}
        onLayerChange={onLayerChange}
        layers={[
          { title: "Add models", content: <p>Search</p> },
          { title: "Review values", content: <p>Prices</p> },
        ]}
      />,
    );
    await frames();
    const back = screen.getByRole("button", { name: "Back" });
    expect(back).toBeDisabled();
    await userEvent.click(back);
    expect(onLayerChange).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog")).toHaveAccessibleName("Review values");
  });
  it("keep an uncontrolled dialog open too", async () => {
    render(
      <Dialog defaultOpen busy title="Rename key">
        <p>Saving</p>
      </Dialog>,
    );
    await frames();
    await userEvent.keyboard("{Escape}");
    await frames();
    expect(screen.getByRole("dialog", { name: "Rename key" })).toBeInTheDocument();
  });
  it("include a step dialog while it submits, with Back and Cancel waiting", async () => {
    const onOpenChange = vi.fn();
    render(
      <StepDialog
        open
        onOpenChange={onOpenChange}
        submitting
        title="Create key"
        submitLabel="Create key"
        onSubmit={() => undefined}
        steps={[{ id: "name", title: "What is it for?", content: <input aria-label="Name" /> }]}
      />,
    );
    await frames();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeDisabled();
    await userEvent.keyboard("{Escape}");
    await frames();
    expect(onOpenChange).not.toHaveBeenCalled();
  });
  it("include an alert dialog while the promise it waits for is pending, with Cancel waiting", async () => {
    const onOpenChange = vi.fn();
    let finish: () => void = () => undefined;
    render(
      <AlertDialog
        open
        onOpenChange={onOpenChange}
        title="Delete key?"
        description="Apps using it stop working."
        confirmLabel="Delete key"
        onConfirm={() => new Promise<void>((resolve) => (finish = resolve))}
      />,
    );
    await frames();
    await userEvent.click(screen.getByRole("button", { name: "Delete key" }));
    expect(screen.getByRole("button", { name: "Cancel" })).toBeDisabled();
    await userEvent.keyboard("{Escape}");
    await frames();
    expect(onOpenChange).not.toHaveBeenCalled();
    await act(async () => finish());
    expect(onOpenChange).toHaveBeenCalledWith(false);
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
  it("leaves Enter to a field that takes it, such as one that adds what was typed as a tag", async () => {
    render(
      <StepDialog
        open
        title="Invite members"
        submitLabel="Send"
        onSubmit={() => {}}
        steps={[
          { id: "who", title: "Who?", content: <TagInput aria-label="Email addresses" /> },
          { id: "role", title: "Which role?", content: <input aria-label="Role" /> },
        ]}
      />,
    );
    await frames();
    const dialog = screen.getByRole("dialog");
    await userEvent.type(screen.getByRole("textbox", { name: "Email addresses" }), "ada@example.com{Enter}");
    expect(screen.getByText("ada@example.com")).toBeInTheDocument();
    expect(dialog).toHaveAccessibleDescription("Step 1 of 2");
    await userEvent.keyboard("{Enter}");
    expect(dialog).toHaveAccessibleDescription("Step 2 of 2");
  });
  it("stays on its step on the Enter that confirms what an input method composed, and on Enter in a search field", async () => {
    render(
      <StepDialog
        open
        title="Add models"
        submitLabel="Add models"
        onSubmit={() => {}}
        steps={[
          {
            id: "pick",
            title: "Which models?",
            content: (
              <>
                <input aria-label="Display name" />
                <input aria-label="Search models" type="search" />
              </>
            ),
          },
          { id: "review", title: "Add them?", content: <p>2 models</p> },
        ]}
      />,
    );
    await frames();
    const dialog = screen.getByRole("dialog");
    const name = screen.getByRole("textbox", { name: "Display name" });
    fireEvent.keyDown(name, { key: "Enter", isComposing: true });
    fireEvent.keyDown(name, { key: "Enter", keyCode: 229 });
    await userEvent.type(screen.getByRole("searchbox", { name: "Search models" }), "qwen{Enter}");
    expect(dialog).toHaveAccessibleDescription("Step 1 of 2");
    fireEvent.keyDown(name, { key: "Enter", keyCode: 13 });
    await frames();
    expect(dialog).toHaveAccessibleDescription("Step 2 of 2");
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
  describe("size", () => {
    afterEach(() => vi.unstubAllGlobals());
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
    const shownStep = (dialog: HTMLElement) => [...dialog.querySelectorAll("section")].find((s) => !s.hidden)!;
    const fixedSizes = (dialog: HTMLElement) =>
      [dialog, ...dialog.querySelectorAll<HTMLElement>("*")].filter((el) => el.style.height || el.style.minHeight || el.style.maxHeight || el.style.width);

    it("leaves every step at its own height: no fixed height, shorter and taller, and the drift on step changes only", async () => {
      render(<Heights heights={[40, 300, 40]} />);
      await frames();
      const dialog = screen.getByRole("dialog");
      expect(fixedSizes(dialog)).toEqual([]);
      expect(shownStep(dialog)).not.toHaveAttribute("data-enter");
      await userEvent.click(screen.getByRole("button", { name: "Continue" }));
      expect(fixedSizes(dialog)).toEqual([]);
      expect(shownStep(dialog)).toHaveAttribute("data-enter", "forward");
      expect(screen.getByRole("textbox", { name: "Field 2" })).toHaveFocus();
      await userEvent.click(screen.getByRole("button", { name: "Back" }));
      expect(fixedSizes(dialog)).toEqual([]);
      expect(shownStep(dialog)).toHaveAttribute("data-enter", "back");
      expect(screen.getByRole("textbox", { name: "Field 1" })).toHaveFocus();
    });
    it("leaves as much room under a tall step as above it", () => {
      const css = readFileSync(join(__dirname, "../../styles/kaeso.css"), "utf8");
      expect(css).toMatch(/@utility pt-dialog-top \{\s*padding-top: max\(16px, 12vh\);/);
      expect(css).toMatch(/@utility dialog-limits-top \{\s*max-height: calc\(100dvh - 2 \* max\(16px, 12vh\)\);/);
    });
    it("keeps its top edge between steps: the popup hangs from a fixed top offset, its frame fills it and nothing else pins it", async () => {
      render(<Heights heights={[40, 300]} />);
      await frames();
      const dialog = screen.getByRole("dialog");
      expect(dialog.parentElement).toHaveClass("fixed", "inset-0", "flex", "items-start", "pt-dialog-top", "justify-center");
      expect(dialog.parentElement).not.toHaveClass("items-center");
      expect(dialog).toHaveClass("dialog-limits-top");
      expect(dialog.querySelector("[data-dialog-frame]")).toHaveClass("flex-1", "min-h-0");
      await userEvent.click(screen.getByRole("button", { name: "Continue" }));
      expect(dialog.getAttributeNames().filter((n) => n.startsWith("data-step"))).toEqual([]);
      expect(dialog.style.cssText).not.toMatch(/top|margin|height/);
    });
    it("starts a step at the top of the body", async () => {
      render(<Heights heights={[40, 300]} />);
      await frames();
      const body = screen.getByRole("dialog").querySelector<HTMLElement>("[data-dialog-body]")!;
      body.scrollTop = 120;
      await userEvent.click(screen.getByRole("button", { name: "Continue" }));
      expect(body.scrollTop).toBe(0);
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
      expect(shownStep(dialog)).not.toHaveAttribute("data-enter");
      expect(screen.getByRole("textbox", { name: "Field 2" })).toHaveFocus();
    });
  });
  describe("result", () => {
    function CreateKey() {
      const [created, setCreated] = useState(false);
      return (
        <StepDialog
          open
          title="Create key"
          submitLabel="Create key"
          onSubmit={() => setCreated(true)}
          steps={[{ id: "name", title: "What is it for?", content: <input aria-label="Name" /> }]}
          result={
            created
              ? {
                  title: "Key created",
                  description: "Copy the key now: it's shown once.",
                  size: "lg",
                  content: <button type="button">Copy key</button>,
                  footer: <Button>Done</Button>,
                }
              : undefined
          }
        />
      );
    }
    it("shows what the last step made in the same dialog, at its own size, with the focus and the announcement in it", async () => {
      render(<CreateKey />);
      await frames();
      const dialog = screen.getByRole("dialog");
      expect(dialog).toHaveClass("w-440");
      await userEvent.click(screen.getByRole("button", { name: "Create key" }));
      await frames();
      expect(screen.getByRole("dialog")).toBe(dialog);
      expect(dialog).toHaveAccessibleName("Key created");
      expect(dialog).toHaveAccessibleDescription("Copy the key now: it's shown once.");
      expect(dialog).toHaveClass("w-720");
      expect(dialog.querySelector("[data-dialog-frame]")).toHaveClass("w-720", "self-center");
      expect(screen.queryByRole("textbox", { name: "Name" })).toBeNull();
      expect(screen.queryByRole("button", { name: "Continue" })).toBeNull();
      expect(screen.getByRole("button", { name: "Done" })).toHaveFocus();
      expect(dialog.querySelector("[aria-live]")).toHaveTextContent("Key created");
      expect(dialog.querySelector("section")).toHaveAttribute("data-enter", "forward");
    });
  });
});

describe("Dialog anchor", () => {
  it("centers by default and hangs from a fixed top edge with anchor top", () => {
    const { rerender } = render(
      <Dialog open title="Rename key">
        <p>Body</p>
      </Dialog>,
    );
    expect(screen.getByRole("dialog").parentElement).toHaveClass("items-center");
    rerender(
      <Dialog open anchor="top" title="Rename key">
        <p>Body</p>
      </Dialog>,
    );
    expect(screen.getByRole("dialog").parentElement).toHaveClass("items-start", "pt-dialog-top");
    expect(screen.getByRole("dialog")).toHaveClass("dialog-limits-top");
  });
});

describe("dialog refs", () => {
  it("reach Dialog's frame when inline", () => {
    const ref = createRef<HTMLDivElement>();
    render(
      <Dialog ref={ref} inline title="Rename key">
        <p>The new name shows in logs.</p>
      </Dialog>,
    );
    expect(ref.current).toHaveAttribute("data-dialog-frame");
    expect(ref.current).toContainElement(screen.getByRole("heading", { name: "Rename key" }));
  });
  it("reach AlertDialog's dialog when open, and its frame when inline", async () => {
    const ref = createRef<HTMLDivElement>();
    const { unmount } = render(
      <AlertDialog ref={ref} open title="Revoke key?" description="Requests with it fail at once." confirmLabel="Revoke key" onConfirm={vi.fn()} />,
    );
    expect(ref.current).toBe(await screen.findByRole("alertdialog", { name: "Revoke key?" }));
    unmount();
    render(<AlertDialog ref={ref} inline title="Revoke key?" description="Requests with it fail at once." confirmLabel="Revoke key" onConfirm={vi.fn()} />);
    expect(ref.current).toContainElement(screen.getByRole("heading", { name: "Revoke key?" }));
    expect(ref.current).toContainElement(screen.getByRole("button", { name: "Revoke key" }));
  });
  it("reach LayerDialog's dialog", async () => {
    const ref = createRef<HTMLDivElement>();
    render(<LayerDialog ref={ref} open layers={[{ title: "Add a provider", content: <p>Choose one to connect.</p> }]} />);
    const dialog = await screen.findByRole("dialog", { name: "Add a provider" });
    expect(ref.current).toBe(dialog);
  });
  it("reach StepDialog's dialog when open, and its frame when inline", async () => {
    const ref = createRef<HTMLDivElement>();
    const steps = [{ id: "name", title: "What is it for?", content: <input aria-label="Name" /> }];
    const { unmount } = render(<StepDialog ref={ref} open title="Create key" submitLabel="Create key" onSubmit={vi.fn()} steps={steps} />);
    expect(ref.current).toBe(await screen.findByRole("dialog", { name: "Create key" }));
    unmount();
    render(<StepDialog ref={ref} inline title="Create key" submitLabel="Create key" onSubmit={vi.fn()} steps={steps} />);
    expect(ref.current).toHaveAttribute("data-dialog-frame");
    expect(ref.current).toContainElement(screen.getByRole("textbox", { name: "Name" }));
  });
});
