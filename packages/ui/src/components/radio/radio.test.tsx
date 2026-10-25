import { createRef } from "react";
import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Radio, RadioCards, RadioGroup } from "./radio.js";

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

  it("names a radio by its label alone and describes it with its description", () => {
    render(
      <>
        <RadioGroup aria-label="Region" defaultValue="eu">
          <Radio value="eu" label="Europe" description="Frankfurt" />
          <Radio value="us" label="United States" description="Virginia" aria-describedby="region-note" />
        </RadioGroup>
        <p id="region-note">Data stays in the region.</p>
        <RadioCards aria-label="Plan" value="team">
          <Radio value="team" label="Team" description="Up to 20 people" />
        </RadioCards>
      </>,
    );
    const europe = screen.getByRole("radio", { name: "Europe" });
    expect(europe).toHaveAccessibleDescription("Frankfurt");
    expect(screen.getByRole("radio", { name: "United States" })).toHaveAccessibleDescription("Data stays in the region. Virginia");
    expect(screen.getByRole("radio", { name: "Team" })).toHaveAccessibleDescription("Up to 20 people");
  });
});

describe("RadioCards' ref", () => {
  it("reaches the radio group", () => {
    const ref = createRef<HTMLDivElement>();
    render(
      <RadioCards ref={ref} aria-label="Plan" value="team">
        <Radio value="team" label="Team" />
        <Radio value="business" label="Business" />
      </RadioCards>,
    );
    expect(ref.current).toBe(screen.getByRole("radiogroup", { name: "Plan" }));
  });
});
