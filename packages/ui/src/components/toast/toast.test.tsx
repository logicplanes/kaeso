import { createRef } from "react";
import { describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { Toast, Toaster, toast } from "./toast.js";

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

describe("Toaster's ref", () => {
  it("reaches the notifications region, also through Toast", () => {
    const ref = createRef<HTMLDivElement>();
    const { unmount } = render(<Toaster ref={ref} />);
    expect(ref.current).toBe(screen.getByRole("region", { name: "Notifications" }));
    unmount();
    render(<Toast ref={ref} />);
    expect(ref.current).toBe(screen.getByRole("region", { name: "Notifications" }));
  });
});

describe("toasts that carry the fix or the way back", () => {
  it("keep an error and an Undo on screen until they are dismissed, while a plain toast leaves after five seconds", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    try {
      render(<Toaster />);
      act(() => {
        toast("Key copied");
        toast.error("Couldn't reach the provider.", { description: "Check the provider key, then try again." });
        toast("Key disabled", { action: { label: "Undo", onClick: () => undefined } });
      });
      await act(() => vi.advanceTimersByTimeAsync(6000));
      expect(screen.queryByText("Key copied")).toBeNull();
      expect(screen.getAllByText("Couldn't reach the provider.").length).toBeGreaterThan(0);
      expect(screen.getByText("Key disabled").closest(".k-toast")).toHaveAttribute("data-stays");
      act(() => toast.dismiss());
    } finally {
      vi.useRealTimers();
    }
  });

  it("still leaves after its own timeout when one is given", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    try {
      render(<Toaster />);
      act(() => {
        toast.error("Couldn't copy the key.", { timeout: 2000 });
      });
      await act(() => vi.advanceTimersByTimeAsync(3000));
      await act(() => vi.advanceTimersByTimeAsync(1000));
      expect(screen.queryByText("Couldn't copy the key.")).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });
});

describe("the toast's close button", () => {
  it("sits at the end of a toast that stays and dismisses it, while a toast that leaves by itself has none", async () => {
    render(<Toaster />);
    act(() => {
      toast("Key copied");
      toast.error("Couldn't copy the key.");
    });
    const error = (await screen.findAllByText("Couldn't copy the key.")).map((el) => el.closest(".k-toast")).find(Boolean)!;
    const copied = screen.getByText("Key copied").closest(".k-toast")!;
    expect(within(copied as HTMLElement).queryByLabelText("Dismiss")).toBeNull();
    await userEvent.click(within(error as HTMLElement).getByLabelText("Dismiss"));
    await waitFor(() => expect(error).not.toBeInTheDocument());
    act(() => toast.dismiss());
  });
});
