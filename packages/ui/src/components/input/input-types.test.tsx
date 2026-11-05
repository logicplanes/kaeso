import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Input } from "./input.js";

describe("Input by its type", () => {
  it("is a search with a clear circle once filled, which Escape also clears", async () => {
    render(<Input type="search" size="sm" aria-label="Search logs" defaultValue="gpt-5" />);
    const search = screen.getByRole("searchbox", { name: "Search logs" });
    expect(screen.getByRole("button", { name: "Clear search" })).toBeInTheDocument();
    search.focus();
    await userEvent.keyboard("{Escape}");
    expect(search).toHaveValue("");
    expect(screen.queryByRole("button", { name: "Clear search" })).toBeNull();
  });

  it("masks a password, offers it to password managers and says when Caps Lock is on", async () => {
    render(<Input type="password" label="Password" />);
    const password = screen.getByLabelText("Password");
    expect(password).toHaveAttribute("type", "password");
    expect(password).toHaveAttribute("autocomplete", "current-password");
    expect(password).not.toHaveAttribute("data-1p-ignore");
    await userEvent.type(password, "{CapsLock}a");
    expect(screen.getByRole("status")).toHaveTextContent("Caps Lock is on.");
    await userEvent.click(screen.getByRole("button", { name: "Show password" }));
    expect(password).toHaveAttribute("type", "text");
  });

  it("masks a secret, keeps it from autocomplete and password managers, and sets it in mono when shown", async () => {
    render(<Input secret aria-label="Provider key" defaultValue="sk-ant-api03" />);
    const key = screen.getByLabelText("Provider key");
    expect(key).toHaveAttribute("type", "password");
    expect(key).toHaveAttribute("autocomplete", "off");
    expect(key).toHaveAttribute("data-1p-ignore");
    expect(screen.queryByRole("status")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Show key" }));
    expect(key).toHaveAttribute("type", "text");
    expect(key).toHaveAttribute("data-mono");
  });

  it("passes none of these props to a plain text field", () => {
    render(<Input aria-label="Key name" shortcut={["mod", "k"]} />);
    const field = screen.getByRole("textbox", { name: "Key name" });
    expect(field).not.toHaveAttribute("shortcut");
    expect(screen.queryByRole("button")).toBeNull();
  });
});
