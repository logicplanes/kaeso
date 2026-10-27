import { describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Pagination } from "./pagination.js";

function Pages({ start = 1, count = 12 }: { start?: number; count?: number }) {
  const [page, setPage] = useState(start);
  return <Pagination variant="pages" page={page} pageCount={count} onPageChange={setPage} />;
}

const slots = () => screen.getByRole("navigation", { name: "Pagination" }).querySelectorAll(":scope > span > *").length;

describe("Pagination pages", () => {
  it("keeps focus on Next when it reaches the last page, and Next stops moving pages", async () => {
    render(<Pages start={10} />);
    const next = screen.getByRole("button", { name: "Next page" });
    next.focus();
    await userEvent.keyboard("{Enter}{Enter}");
    expect(screen.getByRole("button", { name: "Page 12" })).toHaveAttribute("aria-current", "page");
    expect(next).toHaveFocus();
    expect(next).toHaveAttribute("aria-disabled", "true");
    await userEvent.keyboard("{Enter}");
    expect(screen.getByRole("button", { name: "Page 12" })).toHaveAttribute("aria-current", "page");
  });

  it("keeps focus on Previous on the first page", async () => {
    render(<Pages start={2} />);
    const previous = screen.getByRole("button", { name: "Previous page" });
    previous.focus();
    await userEvent.keyboard("{Enter}");
    expect(previous).toHaveFocus();
    expect(previous).toHaveAttribute("aria-disabled", "true");
  });

  it("always shows seven slots, so Next stays where the pointer is", async () => {
    render(<Pages start={1} />);
    const seen = new Set<number>();
    for (let i = 0; i < 11; i++) {
      seen.add(slots());
      await userEvent.click(screen.getByRole("button", { name: "Next page" }));
    }
    expect([...seen]).toEqual([7]);
    expect(screen.getByRole("button", { name: "Page 12" })).toHaveAttribute("aria-current", "page");
  });

  it("brings a page past the end back to the last page when that page is pressed", async () => {
    const onPageChange = vi.fn();
    const { rerender } = render(<Pagination variant="pages" page={5} pageCount={4} onPageChange={onPageChange} />);
    expect(screen.getByRole("button", { name: "Page 4" })).toHaveAttribute("aria-current", "page");
    await userEvent.click(screen.getByRole("button", { name: "Page 4" }));
    expect(onPageChange).toHaveBeenCalledWith(4);
    rerender(<Pagination variant="pages" page={4} pageCount={4} onPageChange={onPageChange} />);
    await userEvent.click(screen.getByRole("button", { name: "Page 4" }));
    expect(onPageChange).toHaveBeenCalledTimes(1);
  });

  it("shows every page when there are seven or fewer", () => {
    render(<Pages count={3} />);
    expect(screen.getAllByRole("button", { name: /^Page / }).map((b) => b.textContent)).toEqual(["1", "2", "3"]);
  });
});

describe("Pagination cursor", () => {
  it("renders nothing when there is no page either way", () => {
    const { container } = render(<Pagination variant="cursor" />);
    expect(container).toBeEmptyDOMElement();
  });
  it("ignores a press on the unavailable direction", async () => {
    const onPrevious = vi.fn();
    render(<Pagination variant="cursor" hasMore onPrevious={onPrevious} />);
    await userEvent.click(screen.getByRole("button", { name: "Previous page" }));
    expect(onPrevious).not.toHaveBeenCalled();
  });
});

function Rows() {
  const [n, setN] = useState(150);
  return <Pagination hasMore={n < 200} onLoadMore={() => setN((x) => x + 50)} />;
}

describe("Pagination load more", () => {
  it("says all rows are shown and keeps focus there after the last page loads", async () => {
    render(<Rows />);
    const more = screen.getByRole("button", { name: "Load more" });
    more.focus();
    await userEvent.keyboard("{Enter}");
    expect(screen.queryByRole("button", { name: "Load more" })).toBeNull();
    expect(screen.getByText("All rows shown")).toHaveFocus();
  });
  it("renders nothing when there was never more to load", () => {
    const { container } = render(<Pagination />);
    expect(container).toBeEmptyDOMElement();
  });
});
