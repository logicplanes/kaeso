import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Input } from "../input/input.js";
import { Textarea } from "../input/textarea.js";
import { Select } from "../select/select.js";
import { Field, Fieldset } from "./field.js";

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

describe("errors", () => {
  it("shows the error under the control, marks the control and describes it to a screen reader", () => {
    render(<Input label="Monthly budget" error="Enter an amount of 0 or more." />);
    const input = screen.getByLabelText("Monthly budget");
    expect(input).toHaveAttribute("aria-invalid", "true");
    expect(input).toHaveAccessibleDescription("Enter an amount of 0 or more.");
    const message = screen.getByText("Enter an amount of 0 or more.");
    expect(message).not.toHaveClass("sr-only");
    expect(message).toHaveClass("text-danger-text");
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

describe("required", () => {
  it("marks the native control required, with or without a label", () => {
    render(
      <>
        <Input label="Project name" required />
        <Input aria-label="Search projects" required />
        <Textarea label="Notes" required />
        <Textarea aria-label="Reason" required />
      </>,
    );
    expect(screen.getByRole("textbox", { name: "Project name" })).toBeRequired();
    expect(screen.getByRole("textbox", { name: "Search projects" })).toBeRequired();
    expect(screen.getByRole("textbox", { name: "Notes" })).toBeRequired();
    expect(screen.getByRole("textbox", { name: "Reason" })).toBeRequired();
  });

  it("names a fieldset by its legend and describes it by its description", () => {
    render(
      <Fieldset legend="New key" description="Keys authenticate your applications.">
        <Field label="Name">
          <Input />
        </Field>
      </Fieldset>,
    );
    const group = screen.getByRole("group", { name: "New key" });
    expect(group).toHaveAccessibleDescription("Keys authenticate your applications.");
  });

  it("dims a disabled field's label and says when it is optional", () => {
    const { container } = render(
      <Field label="Rate limit" optional disabled>
        <Input />
      </Field>,
    );
    const label = container.querySelector("label");
    expect(label).toHaveAttribute("data-disabled");
    expect(label).toHaveTextContent("Rate limitOptional");
    expect(screen.getByRole("textbox", { name: "Rate limit Optional" })).toBeInTheDocument();
  });
});
