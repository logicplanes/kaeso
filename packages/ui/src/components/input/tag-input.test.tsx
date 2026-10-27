import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRef } from "react";
import { TagInput } from "./tag-input.js";

describe("TagInput", () => {
  it("reports the text typed before it becomes a tag, and an empty draft once it does", async () => {
    const onDraft = vi.fn();
    render(<TagInput aria-label="Emails" onInputValueChange={onDraft} />);
    await userEvent.type(screen.getByRole("textbox", { name: "Emails" }), "ada@example.com");
    expect(onDraft).toHaveBeenLastCalledWith("ada@example.com");
    await userEvent.tab();
    expect(onDraft).toHaveBeenLastCalledWith("");
    expect(screen.getByText("ada@example.com")).toBeInTheDocument();
  });
  it("adds a tag at a separator or Enter, skips duplicates, and removes the last with two presses of Backspace", async () => {
    const onChange = vi.fn();
    render(<TagInput aria-label="Emails" placeholder="Add emails" onValueChange={onChange} />);
    const input = screen.getByRole("textbox", { name: "Emails" });
    expect(input).toHaveAttribute("placeholder", "Add emails");
    await userEvent.type(input, "ada@example.com,lin@example.com{Enter}ada@example.com ");
    expect(onChange).toHaveBeenLastCalledWith(["ada@example.com", "lin@example.com"]);
    expect(input).not.toHaveAttribute("placeholder");
    await userEvent.type(input, "{Backspace}");
    expect(onChange).toHaveBeenLastCalledWith(["ada@example.com", "lin@example.com"]);
    expect(screen.getByRole("status")).toHaveTextContent("lin@example.com selected. Press Backspace to remove it.");
    await userEvent.type(input, "{Backspace}");
    expect(onChange).toHaveBeenLastCalledWith(["ada@example.com"]);
    expect(screen.getByRole("status")).toHaveTextContent("Removed lin@example.com");
  });

  it("moves through the tags with the arrows from the start of the field, and Escape lets go", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<TagInput aria-label="Emails" defaultValue={["ada@example.com", "lin@example.com", "sam@example.com"]} onValueChange={onChange} />);
    const input = screen.getByRole("textbox", { name: "Emails" });
    await user.click(input);
    await user.keyboard("{ArrowLeft}{ArrowLeft}");
    expect(screen.getByRole("status")).toHaveTextContent("lin@example.com selected");
    expect(screen.getByText("lin@example.com").closest("[data-force]")).toHaveAttribute("data-force", "focus");
    await user.keyboard("{ArrowRight}{ArrowRight}");
    expect(screen.getByRole("status")).toHaveTextContent("");
    await user.keyboard("{ArrowLeft}{Escape}{Delete}");
    expect(onChange).not.toHaveBeenCalled();
    await user.keyboard("{ArrowLeft}{Delete}");
    expect(onChange).toHaveBeenLastCalledWith(["ada@example.com", "lin@example.com"]);
    expect(input).toHaveFocus();
  });

  it("returns focus to the field after a tag's remove button, and says what went", async () => {
    const user = userEvent.setup();
    render(<TagInput aria-label="Emails" defaultValue={["ada@example.com", "lin@example.com"]} />);
    await user.click(screen.getByRole("button", { name: "Remove ada@example.com" }));
    expect(screen.getByRole("textbox", { name: "Emails" })).toHaveFocus();
    expect(screen.getByRole("status")).toHaveTextContent("Removed ada@example.com");
    expect(screen.queryByText("ada@example.com")).toBeNull();
  });

  it("splits pasted lists and adds the draft on blur", async () => {
    const onChange = vi.fn();
    render(<TagInput aria-label="Emails" name="emails" onValueChange={onChange} />);
    const input = screen.getByRole("textbox", { name: "Emails" });
    fireEvent.paste(input, { clipboardData: { getData: () => "a@example.com; b@example.com\nc@example.com" } });
    expect(onChange).toHaveBeenLastCalledWith(["a@example.com", "b@example.com", "c@example.com"]);
    fireEvent.paste(input, { clipboardData: { getData: () => "single" } });
    fireEvent.paste(input, { clipboardData: { getData: () => "e@example.com\tf@example.com" } });
    expect(onChange).toHaveBeenLastCalledWith(["a@example.com", "b@example.com", "c@example.com", "e@example.com", "f@example.com"]);
    await userEvent.clear(input);
    await userEvent.type(input, "d@example.com");
    fireEvent.blur(input);
    expect(onChange).toHaveBeenLastCalledWith(["a@example.com", "b@example.com", "c@example.com", "e@example.com", "f@example.com", "d@example.com"]);
    expect(document.querySelector('input[type="hidden"]')).toHaveValue("a@example.com,b@example.com,c@example.com,e@example.com,f@example.com,d@example.com");
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

  it("splits pasted text only at its own separators", () => {
    const onChange = vi.fn();
    render(<TagInput aria-label="Models" separators={[","]} onValueChange={onChange} />);
    const input = screen.getByRole("textbox", { name: "Models" });
    fireEvent.paste(input, { clipboardData: { getData: () => "gpt 4o" } });
    expect(onChange).not.toHaveBeenCalled();
    fireEvent.paste(input, { clipboardData: { getData: () => "claude-sonnet,gpt-4o" } });
    expect(onChange).toHaveBeenLastCalledWith(["claude-sonnet", "gpt-4o"]);
  });

  it("is required until it holds a tag", () => {
    const { rerender } = render(<TagInput aria-label="Emails" required value={[]} />);
    expect(screen.getByRole("textbox", { name: "Emails" })).toBeRequired();
    rerender(<TagInput aria-label="Emails" required value={["ada@example.com"]} />);
    const input = screen.getByRole("textbox", { name: "Emails" });
    expect(input).not.toHaveAttribute("required");
    expect(input).toHaveAttribute("aria-required", "true");
  });
});
