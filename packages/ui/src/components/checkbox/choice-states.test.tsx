import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Checkbox, CheckboxGroup } from "./checkbox.js";
import { Radio, RadioCards, RadioGroup } from "../radio/radio.js";
import { Switch } from "../switch/switch.js";
import { Field } from "../field/field.js";

const css = readFileSync(join(__dirname, "../../styles/kaeso.css"), "utf8");
const block = (name: string) => {
  const start = css.indexOf(`@utility ${name} {`);
  return css.slice(start, css.indexOf("\n}\n", start));
};

describe("the choice mark", () => {
  const mark = block("choice-mark");
  it("answers hover and press from the control and from its whole label row", () => {
    expect(mark).toContain(":where([data-choice-row]:hover) :where([data-choice]:not([data-disabled], [data-readonly])) > &");
    expect(mark).toContain(":where([data-choice-row]:active) :where([data-choice]:not([data-disabled], [data-readonly])) > &");
  });
  it("gives a checked mark its own hover and pressed tones, and settles it only with motion allowed", () => {
    expect(mark).toMatch(/--choice-on-hover: var\(--k-ink-hover\)/);
    expect(mark).toMatch(/--choice-on-pressed: var\(--k-ink-pressed\)/);
    expect(mark).toMatch(/@media \(prefers-reduced-motion: no-preference\) \{\s*scale: 0\.9;/);
  });
  it("keeps every state in forced colours: system colours on the mark, focus in Highlight", () => {
    const forced = mark.slice(mark.indexOf("@media (forced-colors: active)"));
    expect(forced).toContain("forced-color-adjust: none");
    expect(forced).toContain("--choice-on: Highlight");
    expect(forced).toContain("--choice-ring: inset 0 0 0 1px CanvasText");
    expect(forced).toContain("--choice-on-disabled: GrayText");
    expect(forced).toContain("--choice-focus: Highlight");
    expect(forced).toContain("outline-style: none");
  });
  it("draws the check when it becomes checked, not when the box first appears", () => {
    const draw = block("check-draw");
    expect(draw).not.toMatch(/animation/);
    expect(draw).toMatch(/:where\(\[data-checked\]\) & \{[^}]*transition-property: stroke-dashoffset;/);
  });
  it("gives the switch track and the slider forced-colour fills", () => {
    expect(block("choice-track")).toContain("--track-on: Highlight");
    expect(block("choice-thumb")).toContain("scale: 0.85");
    expect(block("slider-range")).toContain("background-color: Highlight");
    expect(block("slider-track")).toContain("box-shadow: inset 0 0 0 1px CanvasText");
  });
});

describe("Checkbox", () => {
  it("is a 24px control around an 18px mark, inside a 32px row that names it", () => {
    render(<Checkbox label="Log request content" description="Prompts are kept for 30 days." />);
    const box = screen.getByRole("checkbox", { name: "Log request content" });
    expect(box).toHaveAccessibleDescription("Prompts are kept for 30 days.");
    expect(box).toHaveClass("size-24", "tap-target");
    expect(box.querySelector(".choice-mark")).toHaveClass("size-18");
    expect(box.closest("label")).toHaveAttribute("data-choice-row");
  });
  it("dims the description with the label when disabled, also inside a disabled group", () => {
    render(
      <CheckboxGroup aria-label="Inputs" disabled>
        <Checkbox value="text" label="Text" description="Plain prompts." />
      </CheckboxGroup>,
    );
    expect(screen.getByText("Text")).toHaveClass("text-fg-disabled");
    expect(screen.getByText("Plain prompts.")).toHaveClass("text-fg-disabled");
  });
  it("takes invalid and the error from a Field, so the message is read with it", () => {
    render(
      <Field error="Accept the terms to continue.">
        <Checkbox label="I accept the terms" />
      </Field>,
    );
    const box = screen.getByRole("checkbox", { name: "I accept the terms" });
    expect(box).toHaveAttribute("aria-invalid", "true");
    expect(box).toHaveAccessibleDescription("Accept the terms to continue.");
  });
});

describe("Radio", () => {
  it("is a 24px control around an 18px dot, like the checkbox", () => {
    render(
      <RadioGroup aria-label="Retention" defaultValue="30">
        <Radio value="30" aria-label="30 days" />
      </RadioGroup>,
    );
    const radio = screen.getByRole("radio", { name: "30 days" });
    expect(radio).toHaveClass("size-24", "tap-target");
    expect(radio).toHaveAttribute("data-choice");
    expect(radio.querySelector("[data-shape=dot]")).toHaveClass("choice-mark", "size-18");
  });
  it("dims labels and descriptions of a disabled group", () => {
    render(
      <RadioGroup aria-label="Region" disabled>
        <Radio value="eu" label="Europe" description="Frankfurt." />
      </RadioGroup>,
    );
    expect(screen.getByText("Europe")).toHaveClass("text-fg-disabled");
    expect(screen.getByText("Frankfurt.")).toHaveClass("text-fg-disabled");
  });
  it("shows a card row's keyboard focus on the row and hovers it on the raised tone", () => {
    render(
      <RadioCards aria-label="Dialect" defaultValue="responses">
        <Radio value="responses" label="Responses" />
        <Radio value="legacy" label="Completions" disabled />
      </RadioCards>,
    );
    const row = screen.getByText("Responses").closest("label");
    expect(row).toHaveClass("focus-ring-card-row-within", "hover:bg-raised-hover", "active:bg-raised-pressed");
    expect(screen.getByText("Completions").closest("label")).not.toHaveClass("hover:bg-raised-hover");
  });
});

describe("Switch", () => {
  it("while loading, is busy at once and shows a spinner in the track only after 300ms", () => {
    vi.useFakeTimers();
    try {
      render(<Switch aria-label="Send budget alerts" loading />);
      const track = screen.getByRole("switch", { name: "Send budget alerts" });
      expect(track).toHaveAttribute("aria-busy", "true");
      expect(track.querySelector(".k-spinner")).toBeNull();
      act(() => vi.advanceTimersByTime(300));
      expect(track.querySelector(".k-spinner")).not.toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });
  it("with a label, shows the reason it is off under the label and reads it", async () => {
    const onChange = vi.fn();
    render(<Switch label="Require single sign-on" disabled disabledReason="Connect an identity provider first" onCheckedChange={onChange} />);
    const track = screen.getByRole("switch", { name: "Require single sign-on" });
    expect(track).toHaveAccessibleDescription("Connect an identity provider first");
    expect(screen.getAllByText("Connect an identity provider first")[0]).toHaveClass("text-fg-secondary");
    await userEvent.click(track);
    expect(onChange).not.toHaveBeenCalled();
  });
});
