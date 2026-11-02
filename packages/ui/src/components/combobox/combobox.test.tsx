import { createRef } from "react";
import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Combobox, Highlight } from "./combobox.js";

const PEOPLE = [
  { value: "u1", label: "Ada Okafor" },
  { value: "u2", label: "Lin Park" },
  { value: "u3", label: "Grace Hopper" },
];

describe("Combobox", () => {
  it("takes the first match with Enter while typing", async () => {
    const onChange = vi.fn();
    render(
      <Combobox
        aria-label="Member"
        options={[
          { value: "u1", label: "Ada Okafor" },
          { value: "u2", label: "Lin Park" },
        ]}
        onValueChange={onChange}
      />,
    );
    await userEvent.type(screen.getByRole("combobox", { name: "Member" }), "lin{Enter}");
    expect(onChange).toHaveBeenLastCalledWith("u2");
  });

  it("creates what was typed with Enter when nothing matches", async () => {
    const onCreate = vi.fn();
    render(<Combobox multiple aria-label="Email addresses" options={[]} value={[]} onCreate={onCreate} />);
    await userEvent.type(screen.getByRole("combobox", { name: "Email addresses" }), "lin@example.com{Enter}");
    expect(onCreate).toHaveBeenCalledWith("lin@example.com");
  });

  it("offers to add what was typed, in its own words when it has them", async () => {
    const { rerender } = render(<Combobox aria-label="Teams" options={[]} onCreate={() => {}} />);
    await userEvent.type(screen.getByRole("combobox", { name: "Teams" }), "Billing");
    expect(await screen.findByRole("option", { name: "Add “Billing”" })).toBeInTheDocument();
    rerender(<Combobox aria-label="Teams" options={[]} onCreate={() => {}} createLabel={(label) => `Invite ${label}`} />);
    expect(await screen.findByRole("option", { name: "Invite Billing" })).toBeInTheDocument();
  });

  it("keeps its choice when Escape is pressed with the list closed", async () => {
    const onChange = vi.fn();
    render(
      <Combobox
        aria-label="Model"
        options={[
          { value: "gpt-5", label: "gpt-5" },
          { value: "gpt-5-mini", label: "gpt-5-mini" },
        ]}
        defaultValue="gpt-5"
        onValueChange={onChange}
      />,
    );
    const input = screen.getByRole("combobox", { name: "Model" });
    input.focus();
    await userEvent.keyboard("{Escape}");
    expect(input).toHaveValue("gpt-5");
    expect(onChange).not.toHaveBeenCalled();
  });

  it("keeps earlier choices when a search replaces the options", async () => {
    const onChange = vi.fn();
    const { rerender } = render(
      <Combobox multiple aria-label="Members" options={[{ value: "u1", label: "Ada Okafor" }]} value={["u1"]} onValueChange={onChange} />,
    );
    rerender(<Combobox multiple aria-label="Members" options={[{ value: "u2", label: "Lin Park" }]} value={["u1"]} onValueChange={onChange} />);
    expect(screen.getByLabelText("Ada Okafor")).toBeInTheDocument();
    await userEvent.type(screen.getByRole("combobox", { name: "Members" }), "lin{Enter}");
    expect(onChange).toHaveBeenLastCalledWith(["u1", "u2"]);
  });

  it("lists every option again when opened after a pick", async () => {
    const onChange = vi.fn();
    render(<Combobox aria-label="Member" options={PEOPLE} onValueChange={onChange} />);
    await userEvent.click(screen.getByRole("button", { name: "Show options" }));
    await userEvent.click(await screen.findByRole("option", { name: "Lin Park" }));
    expect(onChange).toHaveBeenLastCalledWith("u2");
    expect(screen.getByRole("combobox", { name: "Member" })).toHaveValue("Lin Park");

    await userEvent.click(screen.getByRole("button", { name: "Show options" }));
    expect((await screen.findAllByRole("option")).map((o) => o.textContent)).toEqual(["Ada Okafor", "Lin Park", "Grace Hopper"]);
  });

  it("lists every option again after a search that led to a pick", async () => {
    render(<Combobox aria-label="Member" options={PEOPLE} />);
    const input = screen.getByRole("combobox", { name: "Member" });
    await userEvent.type(input, "gra");
    const [grace] = await screen.findAllByRole("option");
    expect(grace).toHaveTextContent(/^Grace Hopper$/);
    await userEvent.click(grace!);
    expect(input).toHaveValue("Grace Hopper");

    await userEvent.click(screen.getByRole("button", { name: "Show options" }));
    expect((await screen.findAllByRole("option")).map((o) => o.textContent)).toEqual(["Ada Okafor", "Lin Park", "Grace Hopper"]);
  });

  it("asks for a search only with what was typed, and for none once an option is picked", async () => {
    const onSearch = vi.fn();
    render(<Combobox aria-label="Member" options={PEOPLE} onInputValueChange={onSearch} />);
    await userEvent.type(screen.getByRole("combobox", { name: "Member" }), "li");
    expect(onSearch.mock.calls).toEqual([["l"], ["li"]]);
    const [lin] = await screen.findAllByRole("option");
    expect(lin).toHaveTextContent(/^Lin Park$/);
    await userEvent.click(lin!);
    expect(onSearch).toHaveBeenLastCalledWith("");
    expect(onSearch).not.toHaveBeenCalledWith("Lin Park");
  });

  it("never takes the create row as its own value", async () => {
    const onCreate = vi.fn();
    const onChange = vi.fn();
    const { container } = render(
      <Combobox aria-label="Team" name="team" options={[{ value: "t1", label: "Platform" }]} onCreate={onCreate} onValueChange={onChange} />,
    );
    const input = screen.getByRole("combobox", { name: "Team" });
    await userEvent.type(input, "Billing{Enter}");
    expect(onCreate).toHaveBeenCalledWith("Billing");
    expect(onChange).not.toHaveBeenCalled();
    expect(input).toHaveValue("");
    expect(container.querySelector('input[name="team"]')).toHaveValue("");
  });
});

describe("Highlight", () => {
  it("forwards its ref to the text, with and without a query", () => {
    const ref = createRef<HTMLSpanElement>();
    const { rerender } = render(<Highlight ref={ref} text="gpt-5-mini" query="mini" />);
    expect(ref.current).toHaveTextContent("gpt-5-mini");
    expect(screen.getByText("mini")).toHaveClass("text-fg");
    expect(ref.current).toContainElement(screen.getByText("mini"));
    rerender(<Highlight ref={ref} text="gpt-5-mini" query="" />);
    expect(ref.current).toBe(screen.getByText("gpt-5-mini"));
    expect(ref.current?.tagName).toBe("SPAN");
    expect(ref.current).not.toHaveAttribute("class");
  });
});
