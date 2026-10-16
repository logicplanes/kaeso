import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { SensitiveInput } from "./sensitive-input";
import { SearchInput } from "./input";
import { useState } from "react";
import userEvent from "@testing-library/user-event";

afterEach(() => vi.restoreAllMocks());

describe("InputGroup.Button", () => {
  it("follows the library's own className rule (no layout warning)", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    render(<SensitiveInput label="Password" defaultValue="secret" />);
    expect(screen.getByLabelText("Password")).toBeInTheDocument();
    expect(warn.mock.calls.filter((c) => String(c[0]).includes("className is for layout only"))).toEqual([]);
  });
});

describe("SearchInput", () => {
  it("clears a controlled field through its onChange", async () => {
    function Demo() {
      const [q, setQ] = useState("req_01");
      return <SearchInput aria-label="Search" value={q} onChange={(e) => setQ(e.target.value)} />;
    }
    render(<Demo />);
    await userEvent.click(screen.getByRole("button", { name: "Clear search" }));
    expect(screen.getByRole("searchbox", { name: "Search" })).toHaveValue("");
    expect(screen.getByRole("searchbox", { name: "Search" })).toHaveFocus();
  });
});
