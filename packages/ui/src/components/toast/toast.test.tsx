import { describe, expect, it } from "vitest";
import { act, render, screen } from "@testing-library/react";
import { Toaster, toast } from "./toast.js";

describe("toast", () => {
  it("shows a status message with its action", async () => {
    render(<Toaster />);
    act(() => {
      toast("Key disabled", { action: { label: "Undo", onClick: () => undefined } });
    });
    expect(await screen.findByText("Key disabled")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Undo" })).toBeInTheDocument();
  });
  it("announces errors with high priority", async () => {
    render(<Toaster />);
    act(() => {
      toast.error("Couldn't reach the provider.");
    });
    expect((await screen.findAllByText("Couldn't reach the provider.")).length).toBeGreaterThan(0);
  });
});
