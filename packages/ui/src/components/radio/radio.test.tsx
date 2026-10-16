import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Radio, RadioCards, RadioGroup } from "./radio";

describe("Radio", () => {
  it("chooses one option in a group, with labels and descriptions", async () => {
    const onChange = vi.fn();
    render(
      <RadioGroup aria-label="Region" defaultValue="eu" onValueChange={onChange} orientation="horizontal" name="region">
        <Radio value="eu" label="Europe" description="Frankfurt" />
        <Radio value="us" label="United States" />
        <Radio value="ap" label="Asia Pacific" disabled />
      </RadioGroup>,
    );
    await userEvent.click(screen.getByRole("radio", { name: "United States" }));
    expect(onChange).toHaveBeenCalledWith("us");
    expect(screen.getByRole("radio", { name: "Asia Pacific" })).toHaveAttribute("aria-disabled", "true");
  });

  it("lays out cards with the circle at the end, and a bare circle without a label", async () => {
    const onChange = vi.fn();
    render(
      <>
        <RadioCards aria-label="Plan" value="team" onValueChange={onChange}>
          <Radio value="team" label="Team" description="Up to 20 people" />
          <Radio value="org" label="Organization" disabled />
        </RadioCards>
        <Radio.Group aria-label="Bare" disabled>
          <Radio value="x" aria-label="Only option" id="only" data-force="focus" />
        </Radio.Group>
      </>,
    );
    expect(screen.getByRole("radio", { name: /Team/ })).toHaveAttribute("aria-checked", "true");
    await userEvent.click(screen.getByText("Up to 20 people"));
    expect(document.getElementById("only")).not.toBeNull();
    expect(screen.getByRole("radio", { name: "Only option" })).toHaveAttribute("data-force", "focus");
  });
});
