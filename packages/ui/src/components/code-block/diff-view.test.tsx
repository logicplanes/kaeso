import { describe, expect, it } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DiffView } from "./diff-view.js";

const json = (v: unknown) => JSON.stringify(v, null, 2);
const before = json({ a: 1, b: 2, c: 3, d: 4, e: 5, f: 6, g: 7, name: "Sub Experiment", h: 8 });
const after = json({ a: 1, b: 2, c: 3, d: 4, e: 5, f: 6, g: 7, name: "Agents", h: 8 });

describe("DiffView", () => {
  it("tints removed and added lines and says what changed", () => {
    render(<DiffView before={before} after={after} title="Before → after" lang="json" />);
    const region = screen.getByRole("group", { name: "Before → after" });
    const removed = region.querySelector('[data-diff="removed"]');
    const added = region.querySelector('[data-diff="added"]');
    expect(removed).toHaveClass("bg-diff-removed");
    expect(added).toHaveClass("bg-diff-added");
    expect(removed).toHaveTextContent('Removed "name": "Sub Experiment",');
    expect(added).toHaveTextContent('Added "name": "Agents",');
    expect(screen.getByText("1 line added, 1 removed")).toHaveClass("sr-only");
  });

  it("marks the changed words inside a changed line", () => {
    const { container } = render(<DiffView before={before} after={after} lang="text" />);
    expect([...container.querySelectorAll("[data-diff-change]")].map((m) => [m.textContent, m.className])).toEqual([
      ["Sub Experiment", "rounded-cell forced-colors:underline bg-diff-removed-word"],
      ["Agents", "rounded-cell forced-colors:underline bg-diff-added-word"],
    ]);
  });

  it("highlights the syntax of both versions", async () => {
    const { container } = render(<DiffView before={before} after={after} lang="json" />);
    await waitFor(() => expect(container.querySelector('[data-diff="added"] .text-info-text')).not.toBeNull());
    expect(container.querySelector('[data-diff="removed"] [data-diff-change] .text-info-text')).toHaveTextContent("Sub Experiment");
  });

  it("folds unchanged lines and shows them on request", async () => {
    const user = userEvent.setup();
    render(<DiffView before={before} after={after} />);
    const region = screen.getByRole("group", { name: "Changes" });
    expect(within(region).queryByText(/"a": 1/)).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Show 5 unchanged lines" }));
    expect(within(region).getByText(/"a": 1/)).toBeInTheDocument();
    expect(region).toHaveFocus();
    expect(screen.queryByRole("button", { name: /unchanged/ })).not.toBeInTheDocument();
  });

  it("says when nothing changed", () => {
    render(<DiffView before={before} after={before} />);
    expect(screen.getByText("No changes")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /unchanged lines/ })).toBeNull();
    expect(screen.queryByRole("button", { name: "Copy diff" })).toBeNull();
    expect(screen.getByRole("group", { name: "Changes" }).querySelectorAll("[data-diff=same]")).toHaveLength(11);
  });

  it("shows both line numbers when asked", () => {
    const { container } = render(<DiffView before={"a\nb"} after={"a\nc\nd"} lineNumbers />);
    const numbers = (kind: string) =>
      [...container.querySelectorAll(`[data-diff="${kind}"]`)].map((row) => [...row.querySelectorAll(".tabular")].map((n) => n.textContent));
    expect(numbers("removed")).toEqual([["2", ""]]);
    expect(numbers("added")).toEqual([
      ["", "2"],
      ["", "3"],
    ]);
  });

  it("copies the diff with its markers", async () => {
    const user = userEvent.setup();
    render(<DiffView before={"a\nb"} after={"a\nc"} />);
    await user.click(screen.getByRole("button", { name: "Copy diff" }));
    expect(await navigator.clipboard.readText()).toBe(["  a", "- b", "+ c"].join("\n"));
  });

  it("wraps long lines under their indent", () => {
    const { container } = render(<DiffView before={'  "a": 1'} after={'  "a": 2'} wrap />);
    const row = container.querySelector('[data-diff="added"]');
    expect(row?.lastElementChild).toHaveClass("whitespace-pre-wrap", "pl-16", "-indent-16");
    expect(row?.lastElementChild).toHaveTextContent(/^"a": 2$/);
  });
});
