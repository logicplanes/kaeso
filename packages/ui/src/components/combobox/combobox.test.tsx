import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Combobox } from "./combobox";

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
});
