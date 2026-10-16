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
});
