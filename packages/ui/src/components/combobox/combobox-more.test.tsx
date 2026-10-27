import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Autocomplete, Combobox } from "./combobox.js";
import { Field } from "../field/field.js";
import { icons } from "../../icons/vocabulary.js";

const members = [
  { value: "u1", label: "Ada Okafor", description: "Owner", icon: icons.member },
  { value: "u2", label: "Lin Park" },
  { value: "u3", label: "Sam Reyes", disabled: true },
];

describe("Combobox", () => {
  it("chooses several values as chips and removes one", async () => {
    const onChange = vi.fn();
    render(<Combobox multiple label="Members" options={members} defaultValue={["u1"]} onValueChange={onChange} name="members" />);
    const input = screen.getByRole("combobox", { name: "Members" });
    expect(screen.getByLabelText("Ada Okafor")).toBeInTheDocument();
    await userEvent.type(input, "lin{Enter}");
    expect(onChange).toHaveBeenLastCalledWith(["u1", "u2"]);
  });

  it("follows a controlled multiple value and a controlled single value", async () => {
    const onChange = vi.fn();
    const { rerender } = render(<Combobox multiple aria-label="Members" options={members} value={["u2"]} onValueChange={onChange} />);
    expect(screen.getByLabelText("Lin Park")).toBeInTheDocument();
    rerender(<Combobox aria-label="Owner" options={members} value="u1" onValueChange={onChange} invalid size="sm" />);
    const input = screen.getByRole("combobox", { name: "Owner" });
    expect(input).toHaveValue("Ada Okafor");
    expect(input).toHaveAttribute("aria-invalid", "true");
    rerender(<Combobox aria-label="Owner" options={members} value={null} onValueChange={onChange} />);
    expect(screen.getByRole("combobox", { name: "Owner" })).toHaveValue("");
  });

  it("offers to create what was typed, shows loading and an empty message, and reports the query", async () => {
    const onCreate = vi.fn();
    const onInput = vi.fn();
    const { rerender } = render(<Combobox aria-label="Tag" options={members} onCreate={onCreate} onInputValueChange={onInput} />);
    await userEvent.type(screen.getByRole("combobox", { name: "Tag" }), "ops");
    expect(onInput).toHaveBeenLastCalledWith("ops");
    expect(await screen.findByRole("option", { name: /ops/ })).toBeInTheDocument();
    rerender(<Combobox aria-label="Tag" options={[]} loading emptyText="Nothing" />);
    expect(await screen.findByText("Loading…")).toBeInTheDocument();
    rerender(<Combobox aria-label="Tag" options={[]} emptyText="Nothing" />);
    await waitFor(() => expect(screen.getByText("Nothing")).toBeInTheDocument());
  });

  it("shows the first hundred matches and says how many more there are", async () => {
    const many = Array.from({ length: 250 }, (_, i) => ({ value: `o${i}`, label: `Org ${i}` }));
    render(<Combobox aria-label="Organization" options={many} />);
    await userEvent.click(screen.getByRole("combobox", { name: "Organization" }));
    expect(await screen.findAllByRole("option")).toHaveLength(100);
    expect(screen.getByText("150 more; type to narrow them down")).toBeInTheDocument();
    await userEvent.type(screen.getByRole("combobox"), "Org 24");
    await waitFor(() => expect(screen.getAllByRole("option")).toHaveLength(11));
    expect(screen.queryByText(/more; type/)).not.toBeInTheDocument();
  });

  it("reports the typed text once typing pauses, and keeps a chosen option's label when the options change", async () => {
    const onInput = vi.fn();
    const { unmount } = render(<Combobox aria-label="Organization" options={members} value="u2" searchDelay={200} onInputValueChange={onInput} />);
    const input = screen.getByRole("combobox", { name: "Organization" });
    expect(input).toHaveValue("Lin Park");
    await userEvent.clear(input);
    await userEvent.type(input, "ada");
    expect(onInput).not.toHaveBeenCalled();
    await waitFor(() => expect(onInput).toHaveBeenCalledTimes(1));
    expect(onInput).toHaveBeenCalledWith("ada");
    unmount();
    const { rerender } = render(<Combobox multiple aria-label="Organization" options={members} value={["u2"]} />);
    rerender(<Combobox multiple aria-label="Organization" options={[members[0]!]} value={["u2"]} />);
    expect(screen.getByLabelText("Lin Park")).toBeInTheDocument();
  });

  it("sits in a Field with an error", () => {
    render(
      <Field label="Owner" error="Pick someone who can approve.">
        <Combobox options={members} disabled id="owner" data-force="focus" />
      </Field>,
    );
    expect(screen.getByRole("combobox", { name: "Owner" })).toHaveAttribute("aria-invalid", "true");
  });
});

describe("Combobox right to left", () => {
  it("keeps the room for the chevron on the side the chevron sits, at every size", () => {
    const sizes = ["sm", "md", "lg", "xl"] as const;
    render(
      <div dir="rtl">
        {sizes.map((size) => (
          <Combobox key={size} multiple size={size} aria-label={`Members ${size}`} options={members} defaultValue={["u1", "u2"]} />
        ))}
        <Combobox aria-label="Owner" options={members} defaultValue="u1" />
      </div>,
    );
    const logical = /^(pe|ps|me|ms|end|start)-/;
    const physical = /^(pr|pl|mr|ml|right|left)-/;
    for (const size of sizes) {
      const field = screen.getByRole("combobox", { name: `Members ${size}` }).closest(".material-field")!;
      const chevron = within(field as HTMLElement).getByRole("button", { name: "Show options" });
      const sides = (el: Element) => [...el.classList].filter((c) => logical.test(c) || physical.test(c));
      expect(sides(chevron).every((c) => logical.test(c))).toBe(true);
      expect(sides(field).filter((c) => physical.test(c))).toEqual([]);
      expect(sides(field).some((c) => c.startsWith("pe-"))).toBe(true);
      for (const chip of within(field as HTMLElement).getAllByRole("button", { name: /^Remove / })) {
        expect(sides(chip).filter((c) => physical.test(c))).toEqual([]);
        expect(sides(chip.parentElement!).filter((c) => physical.test(c))).toEqual([]);
      }
    }
    const single = screen.getByRole("combobox", { name: "Owner" }).closest(".material-field")!;
    expect([...single.classList].filter((c) => /^(pr|pl)-/.test(c))).toEqual([]);
  });
});

describe("Autocomplete", () => {
  it("suggests values and keeps any text", async () => {
    const onChange = vi.fn();
    render(
      <Autocomplete label="Model" suggestions={["gpt-5", "gpt-5-mini", "claude-sonnet-4-5"]} onValueChange={onChange} leadingIcon={icons.model} name="model" />,
    );
    const input = screen.getByRole("combobox", { name: "Model" });
    await userEvent.type(input, "gpt");
    const options = await screen.findAllByRole("option");
    expect(options.map((o) => o.textContent)).toEqual(["gpt-5", "gpt-5-mini"]);
    await userEvent.click(options[1]!);
    expect(onChange).toHaveBeenLastCalledWith("gpt-5-mini");
  });

  it("follows a controlled value inside a Field", () => {
    render(
      <Field label="Region" error="Unknown region.">
        <Autocomplete suggestions={["eu-west-1"]} value="mars-1" disabled data-force="focus" id="region" />
      </Field>,
    );
    expect(screen.getByRole("combobox", { name: "Region" })).toHaveValue("mars-1");
    render(<Autocomplete aria-label="Plain" suggestions={[]} defaultValue="draft" size="sm" className="w-200" />);
    expect(screen.getByRole("combobox", { name: "Plain" })).toHaveValue("draft");
  });
});
