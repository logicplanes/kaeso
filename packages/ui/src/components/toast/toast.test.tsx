import { describe, expect, it } from "vitest";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { Toaster, toast } from "./toast.js";

const css = readFileSync(join(__dirname, "../../styles/kaeso.css"), "utf8");
const hoverBridges = [...css.matchAll(/([^{}]*\.k-toast[^{}]*)::after\s*\{([^}]*)\}/g)].map(([, selector = "", body = ""]) => ({
  selector: selector.trim(),
  body,
}));
const catchesThePointerBelow = (toastElement: Element) => hoverBridges.some(({ selector }) => toastElement.matches(selector));

describe("toast", () => {
  it("catches the pointer only between the toasts of an expanded stack, so a button beside or under a toast takes its click", async () => {
    render(<Toaster timeout={0} />);
    act(() => {
      toast("Lin Park is Viewer in this project now");
      toast("Models saved");
    });
    const front = (await screen.findByText("Models saved")).closest(".k-toast")!;
    const behind = screen.getByText("Lin Park is Viewer in this project now").closest(".k-toast")!;
    expect(hoverBridges.length).toBeGreaterThan(0);
    expect(catchesThePointerBelow(front)).toBe(false);
    expect(catchesThePointerBelow(behind)).toBe(false);

    await userEvent.hover(front);
    await waitFor(() => expect(front).toHaveAttribute("data-expanded"));
    expect(catchesThePointerBelow(front)).toBe(false);
    expect(catchesThePointerBelow(behind)).toBe(true);
    for (const { body } of hoverBridges) {
      expect(body).toMatch(/inset-inline: 0;/);
      expect(body).not.toMatch(/width|left|transform/);
    }
    act(() => toast.dismiss());
  });
  it("shows a status message with its action", async () => {
    render(<Toaster />);
    act(() => {
      toast("Key disabled", { action: { label: "Undo", onClick: () => undefined } });
    });
    expect(await screen.findByText("Key disabled")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Undo" })).toBeInTheDocument();
  });
  it("announces errors with high priority", async () => {
    render(<Toaster />);
    act(() => {
      toast.error("Couldn't reach the provider.");
    });
    expect((await screen.findAllByText("Couldn't reach the provider.")).length).toBeGreaterThan(0);
  });
});
