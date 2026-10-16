import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Sheet } from "./sheet";
import { Button } from "../button/button";

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

  it("renders inline with a heading", () => {
    render(
      <Sheet inline open title="Key" size="lg" side="left">
        <p>Details</p>
      </Sheet>,
    );
    expect(screen.getByRole("heading", { name: "Key" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Close" })).toBeInTheDocument();
  });
});
