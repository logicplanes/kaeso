import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { EnvironmentSnippet, SecretReveal } from "./moments.js";
import { RolePicker } from "./role-picker.js";
import { DateRangePicker, DEFAULT_RANGE_PRESETS, DatePicker } from "../date-picker/date-picker.js";
import { FilterChip } from "../filter-chip/filter-chip.js";
import { Field } from "../field/field.js";

describe("SecretReveal", () => {
  it("selects the secret on a click and reports a copy", async () => {
    const user = userEvent.setup();
    const onCopied = vi.fn();
    render(<SecretReveal baseUrl="https://kaeso.example.com" secret="sk-kso-live-1a2b" name="production" snippets={false} onCopied={onCopied} />);
    const field = screen.getByRole("textbox", { name: "Key for production" });
    await user.click(field);
    expect(window.getSelection()?.toString()).toBe("sk-kso-live-1a2b");
    await user.click(screen.getByRole("button", { name: "Copy key" }));
    expect(onCopied).toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Key copied" })).toBeInTheDocument();
    expect(await navigator.clipboard.readText()).toBe("sk-kso-live-1a2b");
  });

  it("names an unnamed secret plainly and shows the environment", () => {
    render(
      <>
        <SecretReveal baseUrl="https://kaeso.example.com" secret="sk" snippets={false} />
        <EnvironmentSnippet baseUrl="https://kaeso.example.com" model="gpt-5" />
      </>,
    );
    expect(screen.getByRole("textbox", { name: "Key" })).toBeInTheDocument();
    expect(screen.getByText("API key variable")).toBeInTheDocument();
    expect(screen.getByText("https://kaeso.example.com/v1")).toBeInTheDocument();
  });
});

describe("RolePicker as a select", () => {
  it("puts built-in roles before custom ones", async () => {
    const onChange = vi.fn();
    render(
      <RolePicker
        appearance="select"
        label="Role"
        name="role"
        size="sm"
        fullWidth
        roles={[
          { value: "support", label: "Support", custom: true },
          { value: "admin", label: "Admin", description: "Everything except ownership" },
        ]}
        value="admin"
        onValueChange={onChange}
      />,
    );
    await userEvent.click(screen.getByRole("combobox", { name: "Role" }));
    const options = await screen.findAllByRole("option");
    expect(options.map((o) => o.textContent?.slice(0, 7))).toEqual(["Admin", "Support"].map((s) => expect.stringContaining(s)) as unknown as string[]);
    await userEvent.click(options[1]!);
    expect(onChange).toHaveBeenCalledWith("support");
  });
});

describe("date range presets", () => {
  it("every default preset ends today or at the end of last month", () => {
    const now = new Date();
    for (const p of DEFAULT_RANGE_PRESETS) {
      const r = p.range();
      expect(r.from!.getTime()).toBeLessThanOrEqual(r.to!.getTime());
    }
    const last = DEFAULT_RANGE_PRESETS.find((p) => p.id === "Last month")!.range();
    expect(last.to!.getDate()).toBe(new Date(now.getFullYear(), now.getMonth(), 0).getDate());
  });

  it("sits in a Field, with the Field's label", () => {
    render(
      <>
        <Field label="Period">
          <DateRangePicker />
        </Field>
        <Field label="Day">
          <DatePicker />
        </Field>
      </>,
    );
    expect(screen.getByRole("button", { name: "Period" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Day" })).toBeInTheDocument();
  });
});

describe("FilterChip choices", () => {
  const values = Array.from({ length: 10 }, (_, i) => ({ value: `m${i}`, label: `model-${i}` }));

  it("changes the operator and toggles several values, searching a long list", async () => {
    const onOperator = vi.fn();
    const onSelected = vi.fn();
    render(
      <FilterChip
        property="Model"
        operator="is"
        operators={["is", "is not"]}
        onOperatorChange={onOperator}
        value="model-1"
        values={values}
        selected={["m1"]}
        multiple
        onSelectedChange={onSelected}
        onRemove={vi.fn()}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "is" }));
    await userEvent.click(await screen.findByRole("menuitemradio", { name: "is not" }));
    expect(onOperator).toHaveBeenCalledWith("is not");
    await userEvent.click(screen.getByRole("button", { name: "model-1" }));
    const search = await screen.findByRole("searchbox", { name: "Search model values" });
    await userEvent.type(search, "9x");
    await userEvent.type(search, "{Backspace}");
    await userEvent.click(screen.getByRole("menuitemcheckbox", { name: "model-9" }));
    expect(onSelected).toHaveBeenLastCalledWith(["m1", "m9"]);
  });

  it("chooses one value, and unchecks the chosen one", async () => {
    const onSelected = vi.fn();
    const { rerender } = render(
      <FilterChip property="Status" operator="is" value="2xx" values={values.slice(0, 3)} selected={["m0"]} onSelectedChange={onSelected} />,
    );
    await userEvent.click(screen.getByRole("button", { name: "2xx" }));
    await userEvent.click(await screen.findByRole("menuitemradio", { name: "model-2" }));
    expect(onSelected).toHaveBeenLastCalledWith(["m2"]);
    rerender(
      <FilterChip property="Status" operator="is" value="2xx" values={values.slice(0, 3)} selected={["m0", "m1"]} multiple onSelectedChange={onSelected} />,
    );
    await userEvent.click(screen.getByRole("button", { name: "2xx" }));
    await userEvent.click(await screen.findByRole("menuitemcheckbox", { name: "model-0" }));
    expect(onSelected).toHaveBeenLastCalledWith(["m1"]);
  });
});
