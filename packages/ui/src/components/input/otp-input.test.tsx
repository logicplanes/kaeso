import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRef } from "react";
import { OTPInput } from "./otp-input.js";
import { Field } from "../field/field.js";

describe("OTPInput", () => {
  it("keeps digits only, reports each change and calls onComplete when full", async () => {
    const onChange = vi.fn();
    const onComplete = vi.fn();
    render(<OTPInput onValueChange={onChange} onComplete={onComplete} />);
    const input = screen.getByRole("textbox", { name: "Verification code" });
    await userEvent.type(input, "12a3456");
    expect(onChange).toHaveBeenLastCalledWith("123456");
    expect(onComplete).toHaveBeenCalledWith("123456");
    expect(input).toHaveValue("123456");
  });

  it("shows the caret in the next empty cell while focused", async () => {
    const { container } = render(<OTPInput length={4} defaultValue="12" size="sm" />);
    const input = screen.getByRole("textbox");
    expect(container.querySelector("[data-focused]")).toBeNull();
    await userEvent.click(input);
    fireEvent.select(input, { target: { selectionStart: 2 } });
    const cells = container.querySelectorAll("[aria-hidden]");
    expect(cells[2]).toHaveAttribute("data-focused");
    fireEvent.blur(input);
    expect(container.querySelector("[data-focused]")).toBeNull();
  });

  it("follows a controlled value, shows a forced focus and marks itself invalid", () => {
    const onChange = vi.fn();
    const { container } = render(<OTPInput value="9" invalid disabled data-force="focus" onValueChange={onChange} aria-label="Code" />);
    const input = screen.getByRole("textbox", { name: "Code" });
    fireEvent.change(input, { target: { value: "98" } });
    expect(onChange).toHaveBeenCalledWith("98");
    expect(input).toHaveValue("9");
    expect(input).toHaveAttribute("aria-invalid", "true");
    expect(container.querySelector("[data-focused]")).not.toBeNull();
    expect(container.querySelector("[data-disabled]")).not.toBeNull();
  });

  it("takes its name and description from a Field and forwards its ref", () => {
    const ref = createRef<HTMLInputElement>();
    const fnRef = vi.fn();
    const { rerender } = render(
      <Field label="Code" description="From your authenticator app" error="That code has expired.">
        <OTPInput ref={ref} />
      </Field>,
    );
    const input = screen.getByRole("textbox", { name: "Code" });
    expect(input).toBe(ref.current);
    expect(input).toHaveAttribute("aria-invalid", "true");
    rerender(<OTPInput ref={fnRef} />);
    expect(fnRef).toHaveBeenCalled();
  });

  it("fills every cell from a pasted code shown with a space or a dash", () => {
    const onComplete = vi.fn();
    render(<OTPInput onComplete={onComplete} />);
    const input = screen.getByRole("textbox", { name: "Verification code" });
    fireEvent.paste(input, { clipboardData: { getData: () => "418 207" } });
    expect(input).toHaveValue("418207");
    expect(onComplete).toHaveBeenCalledWith("418207");
    fireEvent.paste(input, { clipboardData: { getData: () => "552-019" } });
    expect(input).toHaveValue("552019");
  });

  it("keeps a focused cell while the caret moves back, and replaces the digit there", async () => {
    const user = userEvent.setup();
    const { container } = render(<OTPInput length={4} />);
    const input = screen.getByRole("textbox");
    await user.click(input);
    await user.keyboard("4182");
    const focusedIndex = () => [...container.querySelectorAll("[aria-hidden]")].findIndex((c) => c.hasAttribute("data-focused"));
    expect(focusedIndex()).toBe(3);
    await user.keyboard("{ArrowLeft}{ArrowLeft}");
    expect(focusedIndex()).toBe(1);
    await user.keyboard("{Home}");
    expect(focusedIndex()).toBe(0);
    await user.keyboard("9");
    expect(input).toHaveValue("9182");
    expect(focusedIndex()).toBe(1);
    await user.keyboard("{End}");
    expect(focusedIndex()).toBe(3);
  });

  it("puts the caret on the cell that was clicked", () => {
    const { container } = render(<OTPInput length={4} defaultValue="4182" />);
    const cells = [...container.querySelectorAll<HTMLElement>("[aria-hidden]")];
    cells.forEach((c, i) => {
      c.getBoundingClientRect = () => ({ left: i * 56, right: i * 56 + 48, top: 0, bottom: 48, width: 48, height: 48, x: i * 56, y: 0, toJSON: () => ({}) });
    });
    const input = screen.getByRole("textbox");
    fireEvent.focus(input);
    fireEvent.click(input, { clientX: 70 });
    expect(cells[1]).toHaveAttribute("data-focused");
    expect((input as HTMLInputElement).selectionStart).toBe(1);
    expect((input as HTMLInputElement).selectionEnd).toBe(2);
  });
});
