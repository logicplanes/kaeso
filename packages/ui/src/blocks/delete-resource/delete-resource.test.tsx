import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DeleteResource } from "./delete-resource.js";

describe("DeleteResource", () => {
  it("asks for the resource's name before it deletes", async () => {
    const onConfirm = vi.fn();
    render(
      <DeleteResource
        kind="project"
        name="checkout-assistant"
        consequence="Its keys stop working at once."
        onConfirm={onConfirm}
        trigger={<button type="button">Delete</button>}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Delete" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Delete this project?" });
    expect(dialog).toHaveAccessibleDescription("Its keys stop working at once.");
    const confirm = screen.getByRole("button", { name: "Delete project" });
    expect(confirm).toBeDisabled();
    await userEvent.type(screen.getByRole("textbox", { name: "Type checkout-assistant to confirm" }), "checkout-assistant");
    expect(confirm).toBeEnabled();
    await userEvent.click(confirm);
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it("names its own verb, and with a simple confirm needs no typing", async () => {
    const onConfirm = vi.fn();
    const onOpenChange = vi.fn();
    render(
      <DeleteResource
        kind="key"
        name="production"
        verb="Archive"
        confirm="simple"
        consequence="You can restore it later."
        open
        onOpenChange={onOpenChange}
        onConfirm={onConfirm}
      />,
    );
    expect(await screen.findByRole("alertdialog", { name: "Archive this key?" })).toBeInTheDocument();
    expect(screen.queryByRole("textbox")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Archive key" }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(onOpenChange).toHaveBeenLastCalledWith(false));
  });

  it("draws inline for review pages", () => {
    render(<DeleteResource kind="organization" name="northwind" consequence="Every project goes with it." onConfirm={() => undefined} inline />);
    expect(screen.getByText("Delete this organization?")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Delete organization" })).toBeDisabled();
  });
});
