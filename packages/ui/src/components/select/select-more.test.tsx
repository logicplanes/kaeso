import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it, vi } from "vitest";
import { createRef } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Select } from "./select.js";
import { Field } from "../field/field.js";
import { icons } from "../../icons/vocabulary.js";

const options = [
  { value: "gpt-5", label: "gpt-5", group: "OpenAI", description: "Most capable", icon: icons.model },
  { value: "gpt-5-mini", label: "gpt-5-mini", group: "OpenAI" },
  { value: "claude", label: <b>Claude</b>, textValue: "Claude Sonnet 4.5", group: "Anthropic" },
  { value: "old", label: "Retired", disabled: true },
];

describe("Select", () => {
  it("chooses one value from grouped options", async () => {
    const onChange = vi.fn();
    const onOpenChange = vi.fn();
    render(
      <Select
        aria-label="Model"
        options={options}
        placeholder="Choose a model"
        onValueChange={onChange}
        onOpenChange={onOpenChange}
        icon={icons.model}
        name="model"
      />,
    );
    const trigger = screen.getByRole("combobox", { name: "Model" });
    expect(trigger).toHaveTextContent("Choose a model");
    await userEvent.click(trigger);
    expect(onOpenChange).toHaveBeenCalledWith(true);
    expect(await screen.findByText("Anthropic")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("option", { name: /gpt-5-mini/ }));
    expect(onChange).toHaveBeenCalledWith("gpt-5-mini");
  });

  it("keeps a required form from submitting until a value is chosen", async () => {
    const { container } = render(
      <form>
        <Select label="Region" required name="region" options={[{ value: "eu", label: "Europe" }]} />
      </form>,
    );
    const form = container.querySelector("form")!;
    expect(screen.getByRole("combobox", { name: /Region/ })).toHaveAttribute("aria-required", "true");
    expect(form.checkValidity()).toBe(false);
    await userEvent.click(screen.getByRole("combobox", { name: /Region/ }));
    await userEvent.click(await screen.findByRole("option", { name: "Europe" }));
    expect(screen.getByRole("combobox", { name: /Region/ })).toHaveTextContent("Europe");
    expect(form.checkValidity()).toBe(true);
  });

  it("names several chosen values by the first and a count", () => {
    const { rerender } = render(<Select multiple aria-label="Models" options={options} value={["gpt-5", "claude", "gpt-5-mini"]} />);
    expect(screen.getByRole("combobox", { name: "Models" })).toHaveTextContent("gpt-5 +2");
    rerender(<Select multiple aria-label="Models" options={options} value={["claude"]} />);
    expect(screen.getByRole("combobox", { name: "Models" })).toHaveTextContent("Claude Sonnet 4.5");
    rerender(<Select multiple aria-label="Models" options={options} value={[]} placeholder="Any model" />);
    expect(screen.getByRole("combobox", { name: "Models" })).toHaveTextContent("Any model");
  });

  it("shows an unknown value as written, marks errors, and forwards refs", () => {
    const ref = createRef<HTMLButtonElement>();
    const fnRef = vi.fn();
    const { rerender } = render(<Select ref={ref} aria-label="Model" options={options} value="unknown" invalid data-force="focus" id="m" />);
    expect(screen.getByRole("combobox", { name: "Model" })).toHaveTextContent("unknown");
    expect(ref.current).toHaveAttribute("aria-invalid", "true");
    rerender(<Select ref={fnRef} aria-label="Model" options={options} defaultValue="gpt-5" />);
    expect(fnRef).toHaveBeenCalled();
    render(
      <Field label="Region" error="Choose a region.">
        <Select options={[{ value: "eu", label: "Europe" }]} open />
      </Field>,
    );
    expect(screen.getByRole("combobox", { name: "Region" })).toHaveAttribute("aria-invalid", "true");
  });

  it("draws its trigger like a text input: the same ring, hover and focus", () => {
    render(<Select aria-label="Account creation" options={[{ value: "invite", label: "Invitation only" }]} defaultValue="invite" />);
    const trigger = screen.getByRole("combobox", { name: "Account creation" });
    expect(trigger).toHaveClass("material-selector");
    expect(trigger).not.toHaveClass("material-secondary", "focus-ring");
    const css = readFileSync(join(__dirname, "../../styles/kaeso.css"), "utf8");
    const block = (name: string) => css.slice(css.indexOf(`@utility ${name} {`), css.indexOf("\n}", css.indexOf(`@utility ${name} {`)));
    const selector = block("material-selector");
    expect(selector).toContain("box-shadow: var(--k-shadow-selector);");
    expect(selector).toContain("box-shadow: var(--k-shadow-selector-hover);");
    expect(selector).toContain("[data-popup-open]");
    const focus = /inset 0 0 0 1px var\(--k-text\),\s+0 0 0 3px color-mix\(in srgb, var\(--k-text\) 10%, transparent\)/;
    expect(block("material-field")).toMatch(focus);
    expect(selector).toMatch(focus);
  });
});
