import { afterEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { InlineCopyText } from "./copy-button.js";
import { lazyModulesSettled } from "../../utils/lazy-module.js";

describe("InlineCopyText", () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });
  it("copies and says so, and its tooltip stays controlled when Copied goes away", async () => {
    Object.defineProperty(navigator, "clipboard", { value: { writeText: vi.fn().mockResolvedValue(undefined) }, configurable: true });
    const errors = vi.spyOn(console, "error").mockImplementation(() => undefined);
    render(<InlineCopyText value="org_123" />);
    await act(() => lazyModulesSettled());
    vi.useFakeTimers({ shouldAdvanceTime: true });
    fireEvent.click(screen.getByRole("button", { name: /org_123/ }));
    await waitFor(() => expect(screen.getByText("Copied", { selector: ".sr-only" })).toBeInTheDocument());
    await act(async () => {
      vi.advanceTimersByTime(1500);
    });
    await waitFor(() => expect(screen.queryByText("Copied", { selector: ".sr-only" })).toBeNull());
    expect(errors.mock.calls.map((c) => String(c[0])).filter((m) => /controlled/.test(m))).toEqual([]);
  });
  it("never grows wider than its container: a long value ends in an ellipsis", () => {
    render(<InlineCopyText value="Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36" />);
    const button = screen.getByRole("button", { name: /Mozilla/ });
    expect(button).toHaveClass("max-w-full-bleed-4", "min-w-0");
    expect(screen.getByText(/^Mozilla/)).toHaveClass("truncate");
  });
  it("shows its copy icon past the text's end without reserving room for it, so the value ends on the row's edge", () => {
    render(<InlineCopyText value="usr_01m5mbctmseffb5006sxv8cn1j" label="Copy account id" />);
    const icon = screen.getByRole("button", { name: /usr_01m5/ }).querySelector("svg");
    expect(icon).toHaveClass("absolute", "start-full", "opacity-0", "group-hover/copy:opacity-100", "group-focus-visible/copy:opacity-100");
    expect(icon).toHaveClass("coarse:static", "coarse:opacity-100");
  });
});
