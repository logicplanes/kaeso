import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { TimeInput } from "./time-input";
import { Field } from "../field/field";

describe("TimeInput", () => {
  it("steps hours and minutes with the arrows, wrapping around", async () => {
    const onChange = vi.fn();
    render(<TimeInput defaultValue="23:59" onValueChange={onChange} utc />);
    await userEvent.type(screen.getByRole("textbox", { name: "Hours" }), "{ArrowUp}");
    expect(onChange).toHaveBeenLastCalledWith("00:59");
    await userEvent.type(screen.getByRole("textbox", { name: "Minutes" }), "{ArrowUp}");
    expect(onChange).toHaveBeenLastCalledWith("00:00");
    await userEvent.type(screen.getByRole("textbox", { name: "Minutes" }), "{ArrowDown}{ArrowLeft}");
    expect(onChange).toHaveBeenLastCalledWith("00:59");
    expect(screen.getByText("UTC")).toBeInTheDocument();
  });

  it("moves to minutes after two hour digits, clamps them, and pads on blur", async () => {
    const onChange = vi.fn();
    render(<TimeInput aria-label="Start" onValueChange={onChange} />);
    const hours = screen.getByRole("textbox", { name: "Hours" });
    const minutes = screen.getByRole("textbox", { name: "Minutes" });
    expect(screen.getByRole("group", { name: "Start" })).toBeInTheDocument();
    fireEvent.change(hours, { target: { value: "7" } });
    expect(onChange).toHaveBeenLastCalledWith("7:00");
    fireEvent.change(hours, { target: { value: "29" } });
    expect(onChange).toHaveBeenLastCalledWith("23:00");
    expect(minutes).toHaveFocus();
    fireEvent.change(minutes, { target: { value: "5" } });
    expect(onChange).toHaveBeenLastCalledWith("23:5");
    fireEvent.blur(minutes);
    expect(onChange).toHaveBeenLastCalledWith("23:05");
    fireEvent.change(minutes, { target: { value: "75" } });
    expect(onChange).toHaveBeenLastCalledWith("23:59");
  });

  it("sits in a Field with its label and error, and turns off when disabled", () => {
    render(<TimeInput label="Daily report" error="Pick a time in the future." disabled data-force="focus" />);
    expect(screen.getByRole("textbox", { name: /Daily report|Hours/ })).toBeDisabled();
    expect(screen.getByText("Pick a time in the future.")).toBeInTheDocument();
    render(
      <Field label="Reset at">
        <TimeInput value="06:30" />
      </Field>,
    );
    expect(screen.getAllByRole("textbox", { name: /Reset at|Hours/ })[0]).toBeInTheDocument();
  });
});
