import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Input } from "../input/input";
import { Select } from "../select/select";
import { Field } from "./field";

describe("labels", () => {
  it("Input's label names its input", () => {
    render(<Input label="Name" description="Shown in request logs." />);
    expect(screen.getByLabelText("Name")).toBeInstanceOf(HTMLInputElement);
    expect(screen.getByLabelText("Name")).toHaveAccessibleDescription("Shown in request logs.");
  });
  it("Select's label names its trigger", () => {
    render(<Select label="Project" options={[{ value: "a", label: "Checkout assistant" }]} defaultValue="a" />);
    expect(screen.getByRole("combobox", { name: /Project/ })).toBeInTheDocument();
  });
  it("Field names the control inside it", () => {
    render(
      <Field label="Email">
        <Input type="email" />
      </Field>,
    );
    expect(screen.getByLabelText("Email")).toBeInstanceOf(HTMLInputElement);
  });
});

describe("errors are toasts", () => {
  it("doesn't draw an error under the field, but marks the control and describes it to a screen reader", () => {
    render(<Input label="Monthly budget" error="Enter an amount of 0 or more." />);
    const input = screen.getByLabelText("Monthly budget");
    expect(input).toHaveAttribute("aria-invalid", "true");
    expect(input).toHaveAccessibleDescription("Enter an amount of 0 or more.");
    expect(screen.getByText("Enter an amount of 0 or more.")).toHaveClass("sr-only");
    expect(screen.queryByRole("alert")).toBeNull();
  });
  it("marks a control invalid without any text", () => {
    render(
      <Field label="Name" invalid>
        <Input />
      </Field>,
    );
    expect(screen.getByLabelText("Name")).toHaveAttribute("aria-invalid", "true");
  });
});
