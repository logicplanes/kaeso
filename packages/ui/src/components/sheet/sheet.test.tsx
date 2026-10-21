import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRef, useState } from "react";
import { Sheet } from "./sheet.js";
import { AlertDialog } from "../dialog/dialog.js";
import { Button } from "../button/button.js";

function MembersPage() {
  const [member, setMember] = useState<string | null>(null);
  return (
    <>
      <Button onClick={() => setMember("Lin Park")}>Open Lin Park</Button>
      <Sheet open={!!member} onOpenChange={(o) => !o && setMember(null)} title={member ?? "Member"}>
        {member && <p>{member} joined on 3 March</p>}
      </Sheet>
    </>
  );
}

describe("Sheet", () => {
  it("keeps showing the row it was opened for while it slides out, after the page has let go of the row", async () => {
    let finish = () => undefined as void;
    const finished = new Promise<void>((resolve) => {
      finish = resolve;
    });
    const spy = vi.spyOn(Element.prototype, "getAnimations").mockImplementation(function (this: Element) {
      return this.closest("[data-ending-style]") ? [{ finished } as unknown as Animation] : [];
    });
    try {
      render(<MembersPage />);
      await userEvent.click(screen.getByRole("button", { name: "Open Lin Park" }));
      await screen.findByRole("dialog", { name: "Lin Park" });
      await userEvent.click(screen.getByRole("button", { name: "Close" }));
      await waitFor(() => expect(document.querySelector("[data-ending-style]")).not.toBeNull());
      const closing = screen.getByRole("dialog", { hidden: true });
      expect(closing).toHaveTextContent("Lin Park");
      expect(closing).toHaveTextContent("Lin Park joined on 3 March");
      expect(closing).not.toHaveTextContent("Member");
      finish();
      await waitFor(() => expect(screen.queryByRole("dialog", { hidden: true })).toBeNull());
    } finally {
      spy.mockRestore();
    }
  });

  it("updates its trigger and close handler while it is closed", async () => {
    function Export({ exporting, onOpenChange }: { exporting: boolean; onOpenChange: (open: boolean) => void }) {
      return (
        <Sheet
          open={false}
          onOpenChange={onOpenChange}
          trigger={<Button disabled={exporting}>{exporting ? "Exporting…" : "Export logs"}</Button>}
          title="Export logs"
        >
          <p>Last 7 days</p>
        </Sheet>
      );
    }
    const first = vi.fn();
    const second = vi.fn();
    const { rerender } = render(<Export exporting={false} onOpenChange={first} />);
    expect(screen.getByRole("button", { name: "Export logs" })).toBeEnabled();
    rerender(<Export exporting onOpenChange={first} />);
    expect(screen.getByRole("button", { name: "Exporting…" })).toBeDisabled();
    rerender(<Export exporting={false} onOpenChange={second} />);
    await userEvent.click(screen.getByRole("button", { name: "Export logs" }));
    expect(second).toHaveBeenCalledWith(true);
    expect(first).not.toHaveBeenCalled();
  });

  it("opens from its trigger with a title, subtitle, actions and footer, and closes", async () => {
    const onOpenChange = vi.fn();
    render(
      <Sheet
        trigger={<Button>Inspect</Button>}
        title="Request"
        subtitle="req_8f2a"
        actions={<Button>Replay</Button>}
        footer={<Button>Done</Button>}
        onOpenChange={onOpenChange}
        size="md"
      >
        <p>Body</p>
      </Sheet>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Inspect" }));
    const sheet = await screen.findByRole("dialog", { name: "Request" });
    expect(sheet).toHaveTextContent("req_8f2a");
    expect(sheet).toHaveAttribute("data-side", "right");
    await userEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(onOpenChange).toHaveBeenLastCalledWith(false);
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("opens from the bottom without a backdrop when it is not modal", async () => {
    render(
      <Sheet defaultOpen modal={false} side="bottom" title="Menu">
        <p>Items</p>
      </Sheet>,
    );
    expect(await screen.findByRole("dialog", { name: "Menu" })).toHaveAttribute("data-side", "bottom");
  });

  it("dims the page behind a confirmation opened from a sheet that isn't modal, and only once over a modal one", async () => {
    const { rerender } = render(
      <Sheet open modal={false} title="checkout-web">
        <AlertDialog
          open
          title="Archive checkout-web?"
          description="Applications using it stop authenticating."
          confirmLabel="Archive key"
          onConfirm={() => undefined}
        />
      </Sheet>,
    );
    expect(await screen.findByRole("alertdialog", { name: "Archive checkout-web?" })).toBeInTheDocument();
    expect(document.querySelectorAll(".bg-overlay")).toHaveLength(1);
    rerender(
      <Sheet open title="checkout-web">
        <AlertDialog
          open
          title="Archive checkout-web?"
          description="Applications using it stop authenticating."
          confirmLabel="Archive key"
          onConfirm={() => undefined}
        />
      </Sheet>,
    );
    await waitFor(() => expect(document.querySelectorAll(".bg-overlay")).toHaveLength(1));
  });

  it("renders inline with a heading, its ref and a close circle that asks to close", async () => {
    const onOpenChange = vi.fn();
    const ref = createRef<HTMLDivElement>();
    render(
      <Sheet ref={ref} inline open onOpenChange={onOpenChange} title="Key" size="lg" side="left">
        <p>Details</p>
      </Sheet>,
    );
    expect(screen.getByRole("heading", { name: "Key" })).toBeInTheDocument();
    expect(ref.current).toContainElement(screen.getByText("Details"));
    await userEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("from the bottom, watches its header, rows and footer so a change of height grows it upwards smoothly; from the side, watches nothing", async () => {
    const watched: Element[] = [];
    vi.stubGlobal(
      "ResizeObserver",
      class {
        observe(el: Element) {
          watched.push(el);
        }
        unobserve() {}
        disconnect() {}
      },
    );
    const { unmount } = render(
      <Sheet inline side="bottom" title="Menu" footer={<Button>Done</Button>}>
        <p>Theme</p>
        <p>Language</p>
      </Sheet>,
    );
    expect(watched.map((el) => el.textContent)).toEqual(["Menu", "Theme", "Language", "Done"]);
    unmount();
    watched.length = 0;
    render(
      <Sheet inline side="right" title="Key">
        <p>Details</p>
      </Sheet>,
    );
    expect(watched).toEqual([]);
    vi.unstubAllGlobals();
  });
});
