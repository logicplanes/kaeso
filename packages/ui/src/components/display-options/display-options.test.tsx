import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DisplayOptions } from "./display-options.js";
import { icons } from "../../icons/vocabulary.js";

afterEach(() => vi.restoreAllMocks());

describe("DisplayOptions", () => {
  it("opens a panel with the view, rows, toggles and properties", async () => {
    const onView = vi.fn();
    const onToggle = vi.fn();
    const onProps = vi.fn();
    render(
      <DisplayOptions
        trigger="ghost"
        view={{
          value: "list",
          options: [
            { value: "list", label: "List" },
            { value: "board", label: "Board" },
          ],
          onChange: onView,
        }}
        rows={[{ label: "Grouping", icon: icons.stack, value: "status", options: [{ value: "status", label: "Status" }], onChange: vi.fn() }]}
        toggles={[{ label: "Show archived", checked: false, onChange: onToggle }]}
        properties={{
          options: [
            { value: "id", label: "ID" },
            { value: "owner", label: "Owner" },
          ],
          value: ["id"],
          onChange: onProps,
        }}
        footer={<span>Reset</span>}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Display options" }));
    await userEvent.click(await screen.findByRole("radio", { name: "Board" }));
    expect(onView).toHaveBeenCalledWith("board");
    await userEvent.click(screen.getByRole("switch", { name: "Show archived" }));
    expect(onToggle).toHaveBeenCalledWith(true);
    await userEvent.click(screen.getByRole("button", { name: "ID" }));
    expect(onProps).toHaveBeenLastCalledWith([]);
    await userEvent.click(screen.getByRole("button", { name: "Owner" }));
    expect(onProps).toHaveBeenLastCalledWith(["id", "owner"]);
    expect(screen.getByText("Reset")).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Grouping" })).toBeInTheDocument();
  });

  it("marks the properties that are on but don't fit, and says so in a sentence", () => {
    const properties = (hidden: string[]) => ({
      options: [
        { value: "stream", label: "Stream" },
        { value: "key", label: "Key" },
        { value: "agent", label: "User agent" },
      ],
      value: ["stream", "key"],
      onChange: vi.fn(),
      hidden,
    });
    const { rerender } = render(<DisplayOptions inline properties={properties(["stream", "key", "agent"])} />);
    expect(screen.getByRole("button", { name: "Stream" })).toHaveAttribute("aria-description", "Doesn't fit at this width");
    expect(screen.getByRole("button", { name: "User agent" })).not.toHaveAttribute("aria-description");
    expect(screen.getByText("Stream and Key don't fit at this width.")).toBeInTheDocument();
    rerender(<DisplayOptions inline properties={properties(["key"])} />);
    expect(screen.getByText("Key doesn't fit at this width.")).toBeInTheDocument();
    rerender(<DisplayOptions inline properties={properties([])} />);
    expect(screen.queryByText(/fit at this width/)).toBeNull();
  });

  it("renders inline and warns when it has nothing to show", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const { container } = render(<DisplayOptions inline />);
    expect(container.firstElementChild).toHaveClass("rounded-popover");
    expect(warn).toHaveBeenCalledWith(expect.stringContaining("DisplayOptions"));
  });
});
