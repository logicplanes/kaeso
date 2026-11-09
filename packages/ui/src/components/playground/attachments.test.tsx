import { describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Attachments, type Attachment } from "./attachments.js";
import { Composer } from "./composer.js";
import { UserMessage } from "./response-view.js";

const PIXEL = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==";
const photo: Attachment = { id: "a1", name: "dock.png", type: "image/png", src: PIXEL };
const invoice: Attachment = { id: "a2", name: "invoice-2026-10.pdf", type: "application/pdf" };
const memo: Attachment = { id: "a3", name: "standup.m4a", type: "audio/mp4" };
const notes: Attachment = { id: "a4", name: "notes.txt", type: "text/plain" };

const file = (name: string, type: string) => new File(["x"], name, { type });

function Attaching({ onSubmit, disabled }: { onSubmit?: (text: string, files: Attachment[]) => void; disabled?: boolean }) {
  const [items, setItems] = useState<Attachment[]>([]);
  return (
    <Composer
      disabled={disabled}
      attachments={items}
      accept="image/*,application/pdf"
      onAttach={(files) =>
        setItems((now) => [
          ...now,
          ...files.map((f, i) => ({ id: `${f.name}${i}`, name: f.name, type: f.type, ...(f.type.startsWith("image/") ? { src: PIXEL } : {}) })),
        ])
      }
      onRemoveAttachment={(id) => setItems((now) => now.filter((a) => a.id !== id))}
      onSubmit={(text) => onSubmit?.(text, items)}
    />
  );
}

describe("Attachments", () => {
  it("shows an image as a thumbnail and every other file by its name, each with a named remove button", async () => {
    const onRemove = vi.fn();
    render(<Attachments items={[photo, invoice, memo, notes]} onRemove={onRemove} />);
    const list = screen.getByRole("list", { name: "Attachments" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(4);
    expect(screen.getByRole("img", { name: "dock.png" })).toHaveAttribute("src", PIXEL);
    expect(screen.getByText("invoice-2026-10.pdf")).toBeInTheDocument();
    expect(screen.getByText("standup.m4a")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Remove dock.png" }));
    await userEvent.click(screen.getByRole("button", { name: "Remove notes.txt" }));
    expect(onRemove.mock.calls).toEqual([["a1"], ["a4"]]);
  });

  it("draws nothing without files, and no remove buttons without onRemove", () => {
    const { container, rerender } = render(<Attachments items={[]} />);
    expect(container).toBeEmptyDOMElement();
    rerender(<Attachments items={[photo, invoice]} align="end" />);
    expect(screen.getByRole("list", { name: "Attachments" })).toHaveClass("justify-end");
    expect(screen.queryByRole("button")).toBeNull();
  });
});

describe("Composer with attachments", () => {
  it("attaches picked files, offers only the accepted types, and sends with only a file", async () => {
    const sent = vi.fn();
    render(<Attaching onSubmit={sent} />);
    const send = screen.getByRole("button", { name: "Send" });
    expect(send).toBeDisabled();
    const input = document.querySelector("input[type=file]") as HTMLInputElement;
    expect(input).toHaveAttribute("accept", "image/*,application/pdf");
    expect(input).toHaveAttribute("multiple");
    await userEvent.upload(input, [file("dock.png", "image/png"), file("invoice.pdf", "application/pdf")]);
    expect(screen.getByRole("img", { name: "dock.png" })).toBeInTheDocument();
    expect(screen.getByText("invoice.pdf")).toBeInTheDocument();
    expect(input.value).toBe("");
    await userEvent.click(screen.getByRole("button", { name: "Remove invoice.pdf" }));
    expect(screen.queryByText("invoice.pdf")).toBeNull();
    await userEvent.click(send);
    expect(sent).toHaveBeenCalledWith("", [expect.objectContaining({ name: "dock.png" })]);
  });

  it("opens the file picker from Attach files", async () => {
    render(<Attaching />);
    const input = document.querySelector("input[type=file]") as HTMLInputElement;
    const click = vi.spyOn(input, "click");
    await userEvent.click(screen.getByRole("button", { name: "Attach files" }));
    expect(click).toHaveBeenCalled();
  });

  it("takes files dropped on it, darkening its outline while they hover, and ignores a drag without files", () => {
    render(<Attaching />);
    const form = screen.getByRole("textbox", { name: "Prompt" }).closest("form") as HTMLFormElement;
    const text = { types: ["text/plain"], files: [] };
    fireEvent.dragEnter(form, { dataTransfer: text });
    expect(form).not.toHaveAttribute("data-drop");
    const dropped = file("receipt.png", "image/png");
    const files = { types: ["Files"], files: [dropped], dropEffect: "none" };
    fireEvent.dragEnter(form, { dataTransfer: files });
    expect(form).toHaveAttribute("data-drop");
    fireEvent.dragOver(form, { dataTransfer: files });
    fireEvent.dragOver(form, { dataTransfer: text });
    fireEvent.dragLeave(form, { dataTransfer: files, relatedTarget: document.body });
    expect(form).not.toHaveAttribute("data-drop");
    fireEvent.dragEnter(form, { dataTransfer: files });
    fireEvent.drop(form, { dataTransfer: { types: ["Files"], files: [] } });
    expect(form).not.toHaveAttribute("data-drop");
    fireEvent.drop(form, { dataTransfer: files });
    expect(screen.getByRole("img", { name: "receipt.png" })).toBeInTheDocument();
  });

  it("attaches pasted files, and keeps pasted text as text", () => {
    render(<Attaching />);
    const box = screen.getByRole("textbox", { name: "Prompt" });
    const image = file("screenshot.png", "image/png");
    const onlyFile = fireEvent.paste(box, { clipboardData: { files: [image], getData: () => "" } });
    expect(onlyFile).toBe(false);
    expect(screen.getByRole("img", { name: "screenshot.png" })).toBeInTheDocument();
    const withText = fireEvent.paste(box, { clipboardData: { files: [file("chart.png", "image/png")], getData: () => "Q3 chart" } });
    expect(withText).toBe(true);
    expect(fireEvent.paste(box, { clipboardData: { files: [], getData: () => "plain" } })).toBe(true);
    expect(screen.getAllByRole("listitem")).toHaveLength(2);
  });

  it("takes no files while disabled and hides remove buttons", () => {
    render(<Composer disabled attachments={[photo]} onAttach={() => undefined} onRemoveAttachment={() => undefined} />);
    expect(screen.getByRole("button", { name: "Attach files" })).toBeDisabled();
    expect(screen.queryByRole("button", { name: "Remove dock.png" })).toBeNull();
    const form = screen.getByRole("textbox", { name: "Prompt" }).closest("form") as HTMLFormElement;
    fireEvent.dragEnter(form, { dataTransfer: { types: ["Files"], files: [] } });
    expect(form).not.toHaveAttribute("data-drop");
  });

  it("has no Attach button without onAttach", () => {
    render(<Composer />);
    expect(screen.queryByRole("button", { name: "Attach files" })).toBeNull();
  });
});

describe("UserMessage with attachments", () => {
  it("shows the files above the prompt's bubble, and only the files for a prompt without text", () => {
    const { rerender } = render(<UserMessage attachments={[photo, invoice]}>What is on this invoice?</UserMessage>);
    const message = screen.getByRole("group", { name: "Prompt" });
    expect(within(message).getByRole("img", { name: "dock.png" })).toBeInTheDocument();
    expect(within(message).getByText("What is on this invoice?").tagName).toBe("P");
    rerender(<UserMessage attachments={[photo]}>{""}</UserMessage>);
    expect(within(message).queryByRole("paragraph")).toBeNull();
    expect(message.querySelector("p")).toBeNull();
  });
});
