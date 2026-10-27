import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { SensitiveInput } from "./sensitive-input.js";
import { Input, SearchInput } from "./input.js";
import { InputGroup } from "./input-group.js";
import { icons } from "../../icons/vocabulary.js";
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

describe("InputGroup state", () => {
  it("tells the input it is invalid and turns its button off with the group", () => {
    render(
      <InputGroup invalid disabled trailingButton>
        <InputGroup.Input aria-label="Base URL" defaultValue="kaeso.example.com" />
        <InputGroup.Button icon={icons.close} aria-label="Clear" />
      </InputGroup>,
    );
    expect(screen.getByRole("textbox", { name: "Base URL" })).toHaveAttribute("aria-invalid", "true");
    expect(screen.getByRole("textbox", { name: "Base URL" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Clear" })).toBeDisabled();
  });

  it("joins a prefix to the value, so https:// and the host read as one address", () => {
    render(<Input aria-label="Gateway URL" prefix="https://" suffix="/v1" defaultValue="kaeso.example.com" />);
    expect(screen.getByText("https://")).toHaveClass("-me-8");
    expect(screen.getByText("/v1")).not.toHaveClass("-me-8");
  });

  it("hides a search shortcut while the field has focus, keeping its room", () => {
    render(<SearchInput aria-label="Search logs" shortcut={["mod", "k"]} />);
    expect(screen.getByText("K").closest("span[class*='group-focus-within']")).toHaveClass("group-focus-within/field:invisible");
  });
});

describe("SearchInput that cannot change", () => {
  it("offers no clear button while read-only or disabled", () => {
    const { rerender } = render(<SearchInput aria-label="Search logs" defaultValue="gpt-5" readOnly />);
    expect(screen.queryByRole("button", { name: "Clear search" })).toBeNull();
    rerender(<SearchInput aria-label="Search logs" defaultValue="gpt-5" disabled />);
    expect(screen.queryByRole("button", { name: "Clear search" })).toBeNull();
    rerender(<SearchInput aria-label="Search logs" defaultValue="gpt-5" />);
    expect(screen.getByRole("button", { name: "Clear search" })).toBeInTheDocument();
  });
});
