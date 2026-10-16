import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRef } from "react";
import { TagInput } from "./tag-input";

describe("TagInput", () => {
  it("adds a tag at a separator or Enter, skips duplicates, and removes the last with Backspace", async () => {
    const onChange = vi.fn();
    render(<TagInput aria-label="Emails" placeholder="Add emails" onValueChange={onChange} />);
    const input = screen.getByRole("textbox", { name: "Emails" });
    expect(input).toHaveAttribute("placeholder", "Add emails");
    await userEvent.type(input, "ada@example.com,lin@example.com{Enter}ada@example.com ");
    expect(onChange).toHaveBeenLastCalledWith(["ada@example.com", "lin@example.com"]);
    expect(input).not.toHaveAttribute("placeholder");
    await userEvent.type(input, "{Backspace}");
    expect(onChange).toHaveBeenLastCalledWith(["ada@example.com"]);
  });

  it("splits pasted lists and adds the draft on blur", async () => {
    const onChange = vi.fn();
    render(<TagInput aria-label="Emails" name="emails" onValueChange={onChange} />);
    const input = screen.getByRole("textbox", { name: "Emails" });
    fireEvent.paste(input, { clipboardData: { getData: () => "a@example.com; b@example.com\nc@example.com" } });
    expect(onChange).toHaveBeenLastCalledWith(["a@example.com", "b@example.com", "c@example.com"]);
    fireEvent.paste(input, { clipboardData: { getData: () => "single" } });
    await userEvent.type(input, "d@example.com");
    fireEvent.blur(input);
    expect(onChange).toHaveBeenLastCalledWith(["a@example.com", "b@example.com", "c@example.com", "d@example.com"]);
    expect(document.querySelector('input[type="hidden"]')).toHaveValue("a@example.com,b@example.com,c@example.com,d@example.com");
  });

  it("removes a tag with its button and keeps a controlled value", async () => {
    const onChange = vi.fn();
    render(<TagInput aria-label="Tags" value={["prod", "eu"]} onValueChange={onChange} />);
    await userEvent.click(screen.getByRole("button", { name: "Remove prod" }));
    expect(onChange).toHaveBeenCalledWith(["eu"]);
    expect(screen.getByText("prod")).toBeInTheDocument();
  });

  it("marks invalid tags and the field, with the first message as the error", () => {
    render(<TagInput label="Emails" defaultValue={["ok@example.com", "nope"]} validate={(t) => (t.includes("@") ? true : `${t} is not an email`)} />);
    expect(screen.getByRole("textbox", { name: "Emails" })).toHaveAttribute("aria-invalid", "true");
    expect(screen.getAllByText("nope is not an email").length).toBeGreaterThan(0);
  });

  it("is read-only when disabled, focuses its input on a press of the frame, and forwards its ref", () => {
    const ref = createRef<HTMLInputElement>();
    const { container, rerender } = render(<TagInput ref={ref} aria-label="Tags" defaultValue={["prod"]} />);
    const frame = container.querySelector("[class*='material-field']")!;
    fireEvent.mouseDown(frame);
    expect(ref.current).toHaveFocus();
    const fnRef = vi.fn();
    rerender(<TagInput ref={fnRef} disabled aria-label="Tags" defaultValue={["prod"]} data-force="hover" />);
    expect(fnRef).toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: "Remove prod" })).toBeNull();
    expect(container.querySelector("[data-disabled]")).toHaveAttribute("data-force", "hover");
  });
});
