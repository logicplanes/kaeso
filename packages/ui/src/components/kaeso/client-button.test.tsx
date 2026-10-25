import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CLIENT_MARKS, ClientButton } from "./client-button.js";

describe("ClientButton", () => {
  it("is a button named after the client that draws its mark", async () => {
    const onClick = vi.fn();
    render(<ClientButton client="cursor" aria-label="Cursor" onClick={onClick} />);
    const button = screen.getByRole("button", { name: "Cursor" });
    expect(button.querySelector("path")).toHaveAttribute("d", CLIENT_MARKS.cursor);
    await userEvent.click(button);
    expect(onClick).toHaveBeenCalledOnce();
  });

  it("draws a different mark for every client", () => {
    expect(new Set(Object.values(CLIENT_MARKS)).size).toBe(Object.keys(CLIENT_MARKS).length);
  });
});
