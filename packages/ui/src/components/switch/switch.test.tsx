import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Switch } from "./switch";
import { Segmented } from "../segmented/segmented";

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
});
