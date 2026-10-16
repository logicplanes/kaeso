import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AlertDialog, Dialog, LayerDialog } from "./dialog.js";
import { Button } from "../button/button.js";

beforeEach(() => vi.useFakeTimers({ toFake: ["requestAnimationFrame", "cancelAnimationFrame"] }));
afterEach(() => vi.useRealTimers());

const frames = () => act(() => vi.runAllTimersAsync());

describe("AlertDialog", () => {
  it("opens from its trigger, confirms and closes", async () => {
    const onConfirm = vi.fn();
    const onOpenChange = vi.fn();
    render(
      <AlertDialog
        trigger={<Button>Archive key</Button>}
        title="Archive production?"
        description="Apps that use it stop working."
        confirmLabel="Archive key"
        tone="warning"
        onConfirm={onConfirm}
        onOpenChange={onOpenChange}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Archive key" }));
    await frames();
    const dialog = screen.getByRole("alertdialog");
    expect(dialog).toHaveAccessibleDescription("Apps that use it stop working.");
    await userEvent.click(screen.getAllByRole("button", { name: "Archive key" }).at(-1)!);
    expect(onConfirm).toHaveBeenCalled();
    expect(onOpenChange).toHaveBeenLastCalledWith(false);
    await frames();
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it("waits for typed confirmation, then stays busy until the promise settles", async () => {
    let finish = () => {};
    const onConfirm = vi.fn(() => new Promise<void>((r) => (finish = r)));
    render(
      <AlertDialog
        defaultOpen
        title="Delete production?"
        description="This can't be undone."
        confirmLabel="Delete key"
        confirmText="production"
        onConfirm={onConfirm}
      />,
    );
    await frames();
    const confirm = screen.getByRole("button", { name: "Delete key" });
    expect(confirm).toBeDisabled();
    const field = screen.getByRole("textbox", { name: "Type production to confirm" });
    await userEvent.type(field, "prod{Enter}");
    expect(onConfirm).not.toHaveBeenCalled();
    await userEvent.type(field, "uction{Enter}");
    expect(onConfirm).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("button", { name: "Delete key" })).toHaveAttribute("aria-busy", "true");
    await act(async () => finish());
    await frames();
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it("confirms once while busy and stays open for another try when the work fails", async () => {
    let fail = () => {};
    const onConfirm = vi.fn(() => new Promise<void>((_, reject) => (fail = () => reject(new Error("Couldn't reach the gateway.")))));
    render(
      <AlertDialog
        defaultOpen
        title="Delete production?"
        description="This can't be undone."
        confirmLabel="Delete key"
        confirmText="production"
        onConfirm={onConfirm}
      />,
    );
    await frames();
    const field = screen.getByRole("textbox", { name: "Type production to confirm" });
    await userEvent.type(field, "production{Enter}{Enter}");
    expect(onConfirm).toHaveBeenCalledTimes(1);
    await act(async () => fail());
    await frames();
    expect(screen.getByRole("alertdialog")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Delete key" })).not.toHaveAttribute("aria-busy", "true");
    await userEvent.type(field, "{Enter}");
    expect(onConfirm).toHaveBeenCalledTimes(2);
  });

  it("keeps a controlled state and renders inline with a custom cancel label", async () => {
    const onConfirm = vi.fn();
    const onOpenChange = vi.fn();
    const { rerender } = render(
      <AlertDialog
        open
        title="Remove member?"
        description="Lin loses access."
        confirmLabel="Remove"
        cancelLabel="Keep"
        tone="info"
        onConfirm={onConfirm}
        onOpenChange={onOpenChange}
      />,
    );
    await frames();
    await userEvent.click(screen.getByRole("button", { name: "Keep" }));
    expect(onOpenChange).toHaveBeenCalledWith(false);
    expect(screen.getByRole("alertdialog")).toBeInTheDocument();
    rerender(<AlertDialog inline title="Remove member?" description="Lin loses access." confirmLabel="Remove" onConfirm={onConfirm} />);
    expect(screen.getByRole("heading", { name: "Remove member?" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Remove" }));
    expect(onConfirm).toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeInTheDocument();
  });
});

describe("Dialog inline and close", () => {
  it("renders in place with a heading, a description and a close button", () => {
    render(
      <Dialog inline size="sm" title="Create key" description="One sentence." footer={<Button>Create key</Button>}>
        <p>Body</p>
      </Dialog>,
    );
    expect(screen.getByRole("heading", { name: "Create key" })).toBeInTheDocument();
    expect(screen.getByText("One sentence.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Close" })).toBeInTheDocument();
  });

  it("hides its close button and closes from Dialog.Close", async () => {
    const onOpenChange = vi.fn();
    render(
      <Dialog
        defaultOpen
        hideClose
        title="Invite people"
        onOpenChange={onOpenChange}
        footer={
          <Dialog.Close>
            <Button>Done</Button>
          </Dialog.Close>
        }
      />,
    );
    expect(screen.queryByRole("button", { name: "Close" })).toBeNull();
    await frames();
    await userEvent.click(screen.getByRole("button", { name: "Done" }));
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });
});

describe("LayerDialog", () => {
  const layers = [
    { title: "Add a model", content: <p>Choose a provider</p> },
    { title: "OpenAI", description: "Pick a model.", content: <p>gpt-5</p>, footer: <Button>Add model</Button> },
  ];

  it("follows a controlled layer and reports Back", async () => {
    const onLayerChange = vi.fn();
    render(<LayerDialog inline layers={layers} layer={1} onLayerChange={onLayerChange} />);
    expect(screen.getByRole("heading", { name: "OpenAI" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(onLayerChange).toHaveBeenCalledWith(0);
    expect(screen.getByRole("heading", { name: "OpenAI" })).toBeInTheDocument();
  });

  it("falls back to the first layer and renders nothing without layers", () => {
    const { container, rerender } = render(<LayerDialog inline layers={layers} layer={7} />);
    expect(screen.getByRole("heading", { name: "Add a model" })).toBeInTheDocument();
    rerender(<LayerDialog inline layers={[]} />);
    expect(container).toBeEmptyDOMElement();
  });
});
