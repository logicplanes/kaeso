import { createRef } from "react";
import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DeleteResource } from "./delete-resource.js";
import { LocaleProvider } from "../../i18n/provider.js";
import de from "../../i18n/locales/de.js";
import zhCN from "../../i18n/locales/zh-CN.js";

describe("DeleteResource", () => {
  it("asks for the resource's name before it deletes", async () => {
    const onConfirm = vi.fn();
    render(
      <DeleteResource
        kind="project"
        name="checkout-assistant"
        consequence="Its keys stop working at once."
        onConfirm={onConfirm}
        trigger={<button type="button">Delete</button>}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Delete" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Delete “checkout-assistant”?" });
    expect(dialog).toHaveAccessibleDescription("Its keys stop working at once.");
    const confirm = screen.getByRole("button", { name: "Delete project" });
    expect(confirm).toBeDisabled();
    await userEvent.type(screen.getByRole("textbox", { name: "Type checkout-assistant to confirm" }), "checkout-assistant");
    expect(confirm).toBeEnabled();
    await userEvent.click(confirm);
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it("names its own verb, and with a simple confirm needs no typing", async () => {
    const onConfirm = vi.fn();
    const onOpenChange = vi.fn();
    render(
      <DeleteResource
        kind="key"
        name="production"
        verb="Archive"
        confirm="simple"
        consequence="You can restore it later."
        open
        onOpenChange={onOpenChange}
        onConfirm={onConfirm}
      />,
    );
    expect(await screen.findByRole("alertdialog", { name: "Archive “production”?" })).toBeInTheDocument();
    expect(screen.queryByRole("textbox")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Archive key" }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(onOpenChange).toHaveBeenLastCalledWith(false));
  });

  it("draws inline for review pages", () => {
    render(<DeleteResource kind="organization" name="northwind" consequence="Every project goes with it." onConfirm={() => undefined} inline />);
    expect(screen.getByText("Delete “northwind”?")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Delete organization" })).toBeDisabled();
  });
});

describe("DeleteResource's ref", () => {
  it("reaches the dialog when open, and the frame when inline", async () => {
    const ref = createRef<HTMLDivElement>();
    const props = { kind: "project", name: "checkout-assistant", consequence: "Its keys stop working at once.", onConfirm: vi.fn() };
    const { unmount } = render(<DeleteResource ref={ref} {...props} open />);
    expect(ref.current).toBe(await screen.findByRole("alertdialog", { name: "Delete “checkout-assistant”?" }));
    unmount();
    render(<DeleteResource ref={ref} {...props} inline />);
    expect(ref.current).toContainElement(screen.getByRole("heading", { name: "Delete “checkout-assistant”?" }));
  });
});

describe("DeleteResource in other languages", () => {
  it("builds the title and the button around the name, the verb and the kind in each language's own order", () => {
    const props = { name: "Checkout", consequence: "…", onConfirm: () => undefined, inline: true } as const;
    const { unmount } = render(
      <LocaleProvider locale="de" messages={de}>
        <DeleteResource {...props} kind="Projekt" verb="archivieren" />
      </LocaleProvider>,
    );
    expect(screen.getByRole("heading", { name: "„Checkout“ archivieren?" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Projekt archivieren" })).toBeInTheDocument();
    unmount();
    render(
      <LocaleProvider locale="zh-CN" messages={zhCN}>
        <DeleteResource {...props} kind="项目" />
      </LocaleProvider>,
    );
    expect(screen.getByRole("heading", { name: "删除“Checkout”？" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "删除项目" })).toBeInTheDocument();
  });
});
