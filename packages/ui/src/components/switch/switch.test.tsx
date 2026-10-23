import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Switch } from "./switch.js";
import { Segmented } from "../segmented/segmented.js";
import { icons } from "../../icons/vocabulary.js";

describe("Switch", () => {
  it("toggles with a click and reports its state", async () => {
    const onChange = vi.fn();
    render(<Switch label="Send budget alerts" onCheckedChange={onChange} />);
    const s = screen.getByRole("switch", { name: "Send budget alerts" });
    expect(s).toHaveAttribute("aria-checked", "false");
    await userEvent.click(s);
    expect(onChange).toHaveBeenLastCalledWith(true);
    expect(s).toHaveAttribute("aria-checked", "true");
  });

  it("with a disabled reason, stays focusable and doesn't change", async () => {
    const onChange = vi.fn();
    render(<Switch aria-label="Require single sign-on" disabled disabledReason="Connect an identity provider first" onCheckedChange={onChange} />);
    const s = screen.getByRole("switch", { name: "Require single sign-on" });
    expect(s).toHaveAttribute("aria-disabled", "true");
    expect(s).not.toHaveAttribute("disabled");
    await userEvent.click(s);
    expect(onChange).not.toHaveBeenCalled();
    expect(s).toHaveAttribute("aria-checked", "false");
  });
});

describe("Segmented", () => {
  it("moves the selection with the arrow keys", async () => {
    const onChange = vi.fn();
    render(
      <Segmented
        aria-label="Period"
        defaultValue="7d"
        onValueChange={onChange}
        options={[
          { value: "24h", label: "24h" },
          { value: "7d", label: "7d" },
          { value: "30d", label: "30d" },
        ]}
      />,
    );
    screen.getByRole("radio", { name: "7d" }).focus();
    await userEvent.keyboard("{ArrowRight}");
    expect(onChange).toHaveBeenLastCalledWith("30d");
  });

  it("names a segment by its label, and an icon-only one by its aria-label", () => {
    render(
      <Segmented
        aria-label="View"
        options={[
          { value: "list", label: "List", "aria-label": "Show as a list" },
          { value: "board", icon: icons.board, "aria-label": "Board" },
        ]}
      />,
    );
    expect(screen.getByRole("radio", { name: "List" })).not.toHaveAttribute("aria-label");
    expect(screen.getByRole("radio", { name: "Board" })).toBeInTheDocument();
  });

  it("names a switch by its label alone and describes it with its description", async () => {
    const onChange = vi.fn();
    render(<Switch label="Send budget alerts" description="An email when spend passes 80% of the budget." onCheckedChange={onChange} />);
    const control = screen.getByRole("switch", { name: "Send budget alerts" });
    expect(control).toHaveAccessibleDescription("An email when spend passes 80% of the budget.");
    await userEvent.click(screen.getByText("An email when spend passes 80% of the budget."));
    expect(onChange).toHaveBeenCalledWith(true);
  });

  it("draws an off switch with a text input's ring and a light thumb, and keeps both while disabled", () => {
    render(
      <>
        <Switch aria-label="Require two-factor authentication" />
        <Switch aria-label="Require single sign-on" disabled />
        <Switch aria-label="No limit" defaultChecked />
      </>,
    );
    for (const name of ["Require two-factor authentication", "Require single sign-on"]) {
      const track = screen.getByRole("switch", { name });
      expect(track).toHaveClass("shadow-selector", "hover:shadow-selector-hover");
      expect(track.className).not.toMatch(/disabled:shadow-none/);
      expect(track.firstElementChild).toHaveClass("bg-switch-thumb");
    }
    const on = screen.getByRole("switch", { name: "No limit" });
    expect(on).toHaveClass("checked:bg-ink", "checked:shadow-none");
    expect(on.firstElementChild).toHaveClass("data-checked:bg-on-ink");
  });
});
