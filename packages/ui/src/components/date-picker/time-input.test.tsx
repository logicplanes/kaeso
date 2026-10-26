import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { TimeInput } from "./time-input.js";
import { Field } from "../field/field.js";

describe("TimeInput", () => {
  it("steps hours and minutes with the arrows, carrying minutes into hours and wrapping the day", async () => {
    const onChange = vi.fn();
    render(<TimeInput defaultValue="23:59" onValueChange={onChange} utc />);
    await userEvent.type(screen.getByRole("textbox", { name: "Hours" }), "{ArrowUp}");
    expect(onChange).toHaveBeenLastCalledWith("00:59");
    await userEvent.type(screen.getByRole("textbox", { name: "Minutes" }), "{ArrowUp}");
    expect(onChange).toHaveBeenLastCalledWith("01:00");
    await userEvent.type(screen.getByRole("textbox", { name: "Minutes" }), "{ArrowDown}{ArrowLeft}");
    expect(onChange).toHaveBeenLastCalledWith("00:59");
    await userEvent.type(screen.getByRole("textbox", { name: "Hours" }), "{ArrowDown}");
    expect(onChange).toHaveBeenLastCalledWith("23:59");
    await userEvent.type(screen.getByRole("textbox", { name: "Minutes" }), "{ArrowUp}");
    expect(onChange).toHaveBeenLastCalledWith("00:00");
    expect(screen.getByText("UTC")).toBeInTheDocument();
  });

  it("moves to minutes after two hour digits, clamps them, and reports only whole times that changed", async () => {
    const onChange = vi.fn();
    render(<TimeInput aria-label="Start" onValueChange={onChange} />);
    const hours = screen.getByRole("textbox", { name: "Hours" });
    const minutes = screen.getByRole("textbox", { name: "Minutes" });
    expect(screen.getByRole("group", { name: "Start" })).toBeInTheDocument();
    fireEvent.change(hours, { target: { value: "7" } });
    expect(hours).toHaveValue("7");
    expect(onChange).not.toHaveBeenCalled();
    fireEvent.blur(hours);
    expect(onChange).toHaveBeenLastCalledWith("07:00");
    expect(hours).toHaveValue("07");
    fireEvent.change(hours, { target: { value: "1" } });
    fireEvent.change(hours, { target: { value: "12" } });
    expect(onChange).toHaveBeenLastCalledWith("12:00");
    expect(hours).toHaveValue("12");
    hours.focus();
    fireEvent.change(hours, { target: { value: "29" } });
    expect(onChange).toHaveBeenLastCalledWith("23:00");
    expect(minutes).toHaveFocus();
    fireEvent.change(minutes, { target: { value: "5" } });
    expect(onChange).toHaveBeenCalledTimes(3);
    fireEvent.blur(minutes);
    expect(onChange).toHaveBeenLastCalledWith("23:05");
    fireEvent.focus(minutes);
    fireEvent.blur(minutes);
    expect(onChange).toHaveBeenCalledTimes(4);
    fireEvent.change(minutes, { target: { value: "75" } });
    expect(onChange).toHaveBeenLastCalledWith("23:59");
  });

  it("is named by its visible label", () => {
    render(
      <>
        <TimeInput label="Daily report" />
        <Field label="Reset at">
          <TimeInput />
        </Field>
      </>,
    );
    expect(screen.getByRole("group", { name: "Daily report" })).toContainElement(screen.getAllByRole("textbox", { name: "Hours" })[0]!);
    expect(screen.getByRole("group", { name: "Reset at" })).toContainElement(screen.getAllByRole("textbox", { name: "Hours" })[1]!);
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
