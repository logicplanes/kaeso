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
  const icon = (name: RegExp) => screen.getByRole("button", { name }).querySelector("svg");

  it("at the end of a settings row, shows its copy icon past the text without reserving room, so the id ends on the row's edge", () => {
    render(
      <div style={{ display: "flex", paddingRight: "20px" }}>
        <span style={{ display: "flex" }}>
          <InlineCopyText value="usr_01m5mbctmseffb5006sxv8cn1j" label="Copy account id" />
        </span>
      </div>,
    );
    expect(icon(/usr_01m5/)).toHaveClass(
      "absolute",
      "start-full",
      "opacity-0",
      "group-hover/copy:opacity-100",
      "group-focus-visible/copy:opacity-100",
      "coarse:opacity-100",
    );
  });

  it("keeps room for its icon when a button follows it, so the icon never lands on the button", () => {
    render(
      <div style={{ display: "flex", paddingRight: "20px" }}>
        <InlineCopyText value="kaeso-verify=ulCEmQL4y35R" label="Copy TXT value" />
        <button type="button">Verify</button>
      </div>,
    );
    expect(icon(/kaeso-verify/)).not.toHaveClass("absolute");
  });

  it("keeps room for its icon inside a sentence, and in a cell without padding for it", () => {
    render(
      <>
        <p style={{ paddingRight: "24px" }}>
          Send <InlineCopyText value="org_01m5" /> with every request.
        </p>
        <table>
          <tbody>
            <tr>
              <td style={{ paddingRight: "12px" }}>
                <InlineCopyText value="req_01m5" />
              </td>
              <td>12:22</td>
            </tr>
          </tbody>
        </table>
      </>,
    );
    expect(icon(/org_01m5/)).not.toHaveClass("absolute");
    expect(icon(/req_01m5/)).not.toHaveClass("absolute");
  });

  it("ignores what sits below it in a column, and gives the room back when something is added after it", async () => {
    function Row({ verify }: { verify: boolean }) {
      return (
        <div style={{ paddingRight: "24px" }}>
          <div style={{ display: "flex" }}>
            <InlineCopyText value="req_01m5mbcwbfe7" />
            {verify && <button type="button">Verify</button>}
          </div>
          <pre>{"{}"}</pre>
        </div>
      );
    }
    const { rerender } = render(<Row verify={false} />);
    expect(icon(/req_01m5/)).toHaveClass("absolute");
    rerender(<Row verify />);
    await waitFor(() => expect(icon(/req_01m5/)).not.toHaveClass("absolute"));
  });

  it("keeps its icon in its own space on touch screens", () => {
    vi.stubGlobal("matchMedia", (query: string) => ({ matches: query === "(pointer: coarse)", addEventListener: () => {}, removeEventListener: () => {} }));
    try {
      render(
        <div style={{ display: "flex", paddingRight: "20px" }}>
          <InlineCopyText value="proj_01m5" />
        </div>,
      );
      expect(icon(/proj_01m5/)).not.toHaveClass("absolute");
    } finally {
      vi.unstubAllGlobals();
    }
  });
});
