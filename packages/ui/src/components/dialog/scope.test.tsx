import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { AlertDialog, Dialog } from "./dialog.js";
import { DialogScope } from "./scope.js";
import { StepDialog } from "./step-dialog.js";
import { Sheet } from "../sheet/sheet.js";
import { CommandPalette } from "../command-palette/command-palette.js";
import { Button } from "../button/button.js";

const SECRET = "sk-kso-example-secret";

beforeEach(() => vi.useFakeTimers({ toFake: ["requestAnimationFrame", "cancelAnimationFrame"] }));
afterEach(() => vi.useRealTimers());

const frames = () => act(() => vi.runAllTimersAsync());

function CreateKeyFlow({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [name, setName] = useState("");
  const [created, setCreated] = useState<string | null>(null);
  if (created)
    return (
      <Dialog open={open} onOpenChange={(o) => !o && onClose()} title="Key created" footer={<Button onClick={onClose}>Done</Button>}>
        <p>{created}</p>
      </Dialog>
    );
  return (
    <StepDialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title="Create key"
      submitLabel="Create key"
      onSubmit={() => setCreated(SECRET)}
      steps={[
        { id: "name", title: "What is it for?", content: <input aria-label="Name" value={name} onChange={(e) => setName(e.target.value)} /> },
        { id: "expiry", title: "When does it expire?", content: <p>In 90 days</p> },
      ]}
    />
  );
}

function KeysPage({ scoped = true }: { scoped?: boolean }) {
  const [open, setOpen] = useState(false);
  const flow = <CreateKeyFlow open={open} onClose={() => setOpen(false)} />;
  return (
    <>
      <Button onClick={() => setOpen(true)}>New key</Button>
      {scoped ? <DialogScope open={open}>{flow}</DialogScope> : flow}
    </>
  );
}

async function createKey() {
  await userEvent.click(screen.getByRole("button", { name: "New key" }));
  await frames();
  await userEvent.type(screen.getByRole("textbox", { name: "Name" }), "checkout-web");
  await userEvent.click(screen.getByRole("button", { name: "Continue" }));
  await frames();
  await userEvent.click(screen.getByRole("button", { name: "Create key" }));
  await frames();
}

function holdAnimations() {
  let finish = () => undefined as void;
  const finished = new Promise<void>((resolve) => {
    finish = resolve;
  });
  const running = { finished } as unknown as Animation;
  const spy = vi.spyOn(Element.prototype, "getAnimations").mockImplementation(function (this: Element) {
    return this.closest("[data-ending-style]") ? [running] : [];
  });
  return {
    finish: async () => {
      finish();
      await frames();
    },
    restore: () => spy.mockRestore(),
  };
}

describe("DialogScope", () => {
  it("opens a flow whose result lived beside its dialog on the first step again, without the secret", async () => {
    render(<KeysPage />);
    await createKey();
    expect(screen.getByRole("dialog", { name: "Key created" })).toHaveTextContent(SECRET);
    await userEvent.click(screen.getByRole("button", { name: "Done" }));
    await frames();
    expect(screen.queryByText(SECRET)).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "New key" }));
    await frames();
    expect(screen.getByRole("dialog", { name: "Create key" })).toHaveTextContent("Step 1 of 2");
    expect(screen.getByRole("textbox", { name: "Name" })).toHaveValue("");
    expect(document.body).not.toHaveTextContent(SECRET);
  });

  it("shows the old result again without a scope, the bug it prevents", async () => {
    render(<KeysPage scoped={false} />);
    await createKey();
    await userEvent.click(screen.getByRole("button", { name: "Done" }));
    await frames();
    await userEvent.click(screen.getByRole("button", { name: "New key" }));
    await frames();
    expect(screen.getByRole("dialog", { name: "Key created" })).toHaveTextContent(SECRET);
  });

  it("starts fresh after cancelling half-way", async () => {
    render(<KeysPage />);
    await userEvent.click(screen.getByRole("button", { name: "New key" }));
    await frames();
    await userEvent.type(screen.getByRole("textbox", { name: "Name" }), "checkout-web");
    await userEvent.click(screen.getByRole("button", { name: "Continue" }));
    await frames();
    await userEvent.keyboard("{Escape}");
    await frames();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "New key" }));
    await frames();
    expect(screen.getByRole("heading", { name: "What is it for?" })).toBeVisible();
    expect(screen.getByRole("textbox", { name: "Name" })).toHaveValue("");
  });

  it("keeps the content while the dialog animates out and forgets it once the dialog is gone", async () => {
    const animations = holdAnimations();
    try {
      render(<KeysPage />);
      await createKey();
      await userEvent.click(screen.getByRole("button", { name: "Done" }));
      await frames();
      expect(screen.getByText(SECRET)).toBeInTheDocument();
      expect(screen.queryByRole("heading", { name: "What is it for?" })).not.toBeInTheDocument();
      await animations.finish();
      await waitFor(() => expect(screen.queryByText(SECRET)).not.toBeInTheDocument());
    } finally {
      animations.restore();
    }
  });

  it("renders nothing while closed and starts each open as a new session", async () => {
    const mounts = vi.fn();
    function Counter() {
      const [id] = useState(() => {
        mounts();
        return mounts.mock.calls.length;
      });
      return (
        <Dialog open title={`Session ${id}`}>
          <p>Body</p>
        </Dialog>
      );
    }
    const { rerender } = render(
      <DialogScope open={false}>
        <Counter />
      </DialogScope>,
    );
    expect(mounts).not.toHaveBeenCalled();
    rerender(
      <DialogScope open>
        <Counter />
      </DialogScope>,
    );
    await frames();
    expect(screen.getByRole("dialog", { name: "Session 1" })).toBeInTheDocument();
    rerender(
      <DialogScope open={false}>
        <Counter />
      </DialogScope>,
    );
    await frames();
    rerender(
      <DialogScope open>
        <Counter />
      </DialogScope>,
    );
    await frames();
    expect(screen.getByRole("dialog", { name: "Session 2" })).toBeInTheDocument();
  });

  it("forgets a sheet's draft and an alert dialog's typed confirmation", async () => {
    function EditSheet({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
      const [draft, setDraft] = useState("");
      return (
        <Sheet open={open} onOpenChange={onOpenChange} title="checkout-web">
          <input aria-label="Description" value={draft} onChange={(e) => setDraft(e.target.value)} />
        </Sheet>
      );
    }
    function Page() {
      const [sheet, setSheet] = useState(false);
      const [alert, setAlert] = useState(false);
      return (
        <>
          <Button onClick={() => setSheet(true)}>Edit</Button>
          <Button onClick={() => setAlert(true)}>Delete</Button>
          <DialogScope open={sheet}>
            <EditSheet open={sheet} onOpenChange={setSheet} />
          </DialogScope>
          <AlertDialog
            open={alert}
            onOpenChange={setAlert}
            title="Delete Northwind Labs?"
            description="Its projects and keys are deleted."
            confirmLabel="Delete organization"
            confirmText="Northwind Labs"
            onConfirm={() => undefined}
          />
        </>
      );
    }
    render(<Page />);
    await userEvent.click(screen.getByRole("button", { name: "Edit" }));
    await frames();
    await userEvent.type(screen.getByRole("textbox", { name: "Description" }), "Checkout");
    await userEvent.keyboard("{Escape}");
    await frames();
    await userEvent.click(screen.getByRole("button", { name: "Edit" }));
    await frames();
    expect(screen.getByRole("textbox", { name: "Description" })).toHaveValue("");
    await userEvent.keyboard("{Escape}");
    await frames();
    await userEvent.click(screen.getByRole("button", { name: "Delete" }));
    await frames();
    await userEvent.type(screen.getByRole("textbox"), "Northwind");
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await frames();
    await userEvent.click(screen.getByRole("button", { name: "Delete" }));
    await frames();
    expect(screen.getByRole("textbox")).toHaveValue("");
    expect(screen.getByRole("button", { name: "Delete organization" })).toBeDisabled();
  });
  it("starts a new session for a flow reopened before its dialog has finished closing", async () => {
    const animations = holdAnimations();
    try {
      render(<KeysPage />);
      await createKey();
      await userEvent.click(screen.getByRole("button", { name: "Done" }));
      await frames();
      expect(screen.getByText(SECRET)).toBeInTheDocument();
      await userEvent.click(screen.getByRole("button", { name: "New key", hidden: true }));
      await frames();
      expect(screen.queryByText(SECRET)).not.toBeInTheDocument();
      expect(screen.getByRole("textbox", { name: "Name" })).toHaveValue("");
    } finally {
      animations.restore();
    }
  });
});

describe("CommandPalette", () => {
  it("keeps its results while closing and opens again with an empty query", async () => {
    const animations = holdAnimations();
    function Host() {
      const [open, setOpen] = useState(true);
      return (
        <>
          <Button onClick={() => setOpen(true)}>Search</Button>
          <CommandPalette
            open={open}
            onOpenChange={setOpen}
            hotkey={false}
            groups={[
              {
                label: "Go to",
                items: [
                  { id: "logs", label: "Request logs", onSelect: () => undefined },
                  { id: "keys", label: "API keys", onSelect: () => undefined },
                ],
              },
            ]}
          />
        </>
      );
    }
    try {
      render(<Host />);
      await frames();
      await userEvent.type(screen.getByRole("combobox"), "logs");
      expect(screen.getAllByRole("option")).toHaveLength(1);
      await userEvent.keyboard("{Escape}");
      await frames();
      expect(screen.getByRole("option", { name: /Request logs/ })).toBeInTheDocument();
      await animations.finish();
      await waitFor(() => expect(screen.queryByRole("combobox")).not.toBeInTheDocument());
      await userEvent.click(screen.getByRole("button", { name: "Search" }));
      await frames();
      expect(screen.getByRole("combobox")).toHaveValue("");
      expect(screen.getAllByRole("option")).toHaveLength(2);
    } finally {
      animations.restore();
    }
  });

  it("opens with an empty query when it opens again before it has finished closing", async () => {
    const animations = holdAnimations();
    function Host() {
      const [open, setOpen] = useState(true);
      return (
        <>
          <Button onClick={() => setOpen(true)}>Search</Button>
          <CommandPalette
            open={open}
            onOpenChange={setOpen}
            hotkey={false}
            groups={[{ label: "Go to", items: [{ id: "logs", label: "Request logs", onSelect: () => setOpen(false) }] }]}
          />
        </>
      );
    }
    try {
      render(<Host />);
      await frames();
      await userEvent.type(screen.getByRole("combobox"), "logs");
      await userEvent.keyboard("{Escape}");
      await frames();
      expect(screen.getByRole("combobox")).toHaveValue("logs");
      await userEvent.click(screen.getByRole("button", { name: "Search", hidden: true }));
      await frames();
      expect(screen.getByRole("combobox")).toHaveValue("");
    } finally {
      animations.restore();
    }
  });
});
