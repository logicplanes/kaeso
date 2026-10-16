import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { NumberInput } from "./number-input.js";
import { LocaleProvider } from "../../i18n/index.js";

describe("NumberInput", () => {
  it("steps exactly with the buttons and the arrows, within its limits", async () => {
    const onChange = vi.fn();
    render(<NumberInput aria-label="Budget" defaultValue="9.5" step="0.5" min="0" max="10" unit="USD" onValueChange={onChange} />);
    const input = screen.getByRole("textbox", { name: "Budget" });
    await userEvent.click(screen.getByRole("button", { name: "Increase" }));
    expect(onChange).toHaveBeenLastCalledWith("10.0");
    await userEvent.click(screen.getByRole("button", { name: "Increase" }));
    expect(onChange).toHaveBeenLastCalledWith("10");
    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(onChange).toHaveBeenLastCalledWith("9.5");
    fireEvent.keyDown(input, { key: "ArrowUp" });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(screen.getByText("USD")).toBeInTheDocument();
  });

  it("groups thousands when not focused, takes only decimals, and clamps on blur", async () => {
    const onChange = vi.fn();
    const onBlur = vi.fn();
    const onFocus = vi.fn();
    render(<NumberInput aria-label="Tokens" max="100000" onValueChange={onChange} onBlur={onBlur} onFocus={onFocus} />);
    const input = screen.getByRole("textbox", { name: "Tokens" });
    await userEvent.type(input, "12a3456");
    expect(onChange).toHaveBeenLastCalledWith("123456");
    expect(onFocus).toHaveBeenCalled();
    fireEvent.blur(input);
    expect(onChange).toHaveBeenLastCalledWith("100000");
    expect(onBlur).toHaveBeenCalled();
    expect(input).toHaveValue("100,000");
  });

  it("keeps a lone minus sign and steps up from nothing", async () => {
    const onChange = vi.fn();
    render(<NumberInput aria-label="Offset" min="-5" onValueChange={onChange} />);
    const input = screen.getByRole("textbox", { name: "Offset" });
    await userEvent.type(input, "-");
    fireEvent.blur(input);
    expect(onChange).toHaveBeenLastCalledWith("-");
    fireEvent.change(input, { target: { value: "-9" } });
    fireEvent.blur(input);
    expect(onChange).toHaveBeenLastCalledWith("-5");
  });

  it("writes German separators and reads a German comma", () => {
    const onChange = vi.fn();
    render(
      <LocaleProvider locale="de" messages={{}}>
        <NumberInput aria-label="Budget" value="1234.5" onValueChange={onChange} />
      </LocaleProvider>,
    );
    const input = screen.getByRole("textbox", { name: "Budget" });
    expect(input).toHaveValue("1.234,5");
    fireEvent.focus(input);
    expect(input).toHaveValue("1234,5");
    fireEvent.change(input, { target: { value: "1234,75" } });
    expect(onChange).toHaveBeenLastCalledWith("1234.75");
  });

  it("hides its stepper when read-only or asked to", () => {
    const { rerender } = render(<NumberInput aria-label="Rate" readOnly value="3" />);
    expect(screen.getByRole("button", { name: "Increase", hidden: true })).toBeDisabled();
    rerender(<NumberInput aria-label="Rate" hideStepper value="3" align="start" />);
    expect(screen.queryByRole("button", { name: "Increase", hidden: true })).toBeNull();
  });
});
