import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Button } from "./button";
import { SwapText } from "../swap-text/swap-text";

describe("Button", () => {
  it("is a pill button that calls onClick", async () => {
    const onClick = vi.fn();
    render(<Button onClick={onClick}>Create key</Button>);
    await userEvent.click(screen.getByRole("button", { name: "Create key" }));
    expect(onClick).toHaveBeenCalledOnce();
    expect(screen.getByRole("button").className).toContain("rounded-pill");
  });
  it("loading keeps the button enabled-looking but inert, and marks it busy", async () => {
    const onClick = vi.fn();
    render(
      <Button loading onClick={onClick}>
        Save
      </Button>,
    );
    const b = screen.getByRole("button");
    expect(b).toHaveAttribute("data-loading");
    expect(b).toHaveAttribute("aria-busy", "true");
    await userEvent.click(b);
    expect(onClick).not.toHaveBeenCalled();
  });
  it("reserves the width of every label it can show", () => {
    const { container } = render(<Button reserveLabels={["Copied"]}>Copy page</Button>);
    expect(container.textContent).toContain("Copied");
  });
});

describe("SwapText", () => {
  it("renders the visible label once for assistive technology and reserves the others hidden", () => {
    render(<SwapText reserve={["Copied"]}>Copy</SwapText>);
    expect(screen.getByText("Copy")).toBeVisible();
    expect(screen.getByText("Copied")).toHaveAttribute("aria-hidden", "true");
  });
});
