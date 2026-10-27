import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { ButtonGroup } from "./button-group.js";
import { icons } from "../../icons/vocabulary.js";

afterEach(() => vi.restoreAllMocks());

describe("ButtonGroup.Item", () => {
  it("warns when an icon-only item has no name, and stays quiet when it has one", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const { rerender } = render(
      <ButtonGroup aria-label="View">
        <ButtonGroup.Item icon={icons.list} />
      </ButtonGroup>,
    );
    expect(warn).toHaveBeenCalledWith(expect.stringContaining("ButtonGroup.Item: an icon-only control needs an aria-label"));
    warn.mockClear();
    rerender(
      <ButtonGroup aria-label="View">
        <ButtonGroup.Item icon={icons.list} aria-label="List" />
      </ButtonGroup>,
    );
    expect(warn).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "List" })).toBeInTheDocument();
  });
});

describe("ButtonGroup selection", () => {
  it("exposes the selected item as pressed, and keeps it pressed when disabled", () => {
    render(
      <ButtonGroup aria-label="Period">
        <ButtonGroup.Item pressed={false}>Day</ButtonGroup.Item>
        <ButtonGroup.Item pressed>Week</ButtonGroup.Item>
        <ButtonGroup.Item pressed={false} disabled>
          Month
        </ButtonGroup.Item>
      </ButtonGroup>,
    );
    expect(screen.getByRole("group", { name: "Period" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Week", pressed: true })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Day", pressed: false })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Month", pressed: false })).toBeDisabled();
  });
});
