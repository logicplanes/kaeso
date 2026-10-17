import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRef } from "react";
import { Sheet } from "./sheet.js";
import { Button } from "../button/button.js";

describe("Sheet", () => {
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
