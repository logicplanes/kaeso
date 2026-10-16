import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Checkbox, CheckboxGroup } from "./checkbox";
import { Accordion, Collapsible } from "../collapsible/collapsible";
import { SensitiveInput } from "../input/sensitive-input";
import { Popover } from "../popover/popover";
import { HoverCard } from "../popover/hover-card";
import { focusInputOnPress } from "../../utils/field-press";

describe("Checkbox", () => {
  it("checks with a label and description, and reports the state", async () => {
    const onChange = vi.fn();
    render(<Checkbox label="Send a weekly summary" description="Every Monday" onCheckedChange={onChange} name="weekly" value="yes" required />);
    await userEvent.click(screen.getByRole("checkbox", { name: /Send a weekly summary/ }));
    expect(onChange).toHaveBeenCalledWith(true);
  });

  it("follows controlled, indeterminate, invalid, read-only and disabled states", () => {
    render(
      <>
        <Checkbox aria-label="All" indeterminate checked={false} invalid data-force="focus" id="all" />
        <Checkbox label="Locked" disabled defaultChecked />
        <Checkbox aria-label="Read only" readOnly checked />
      </>,
    );
    expect(screen.getByRole("checkbox", { name: "All" })).toHaveAttribute("aria-invalid", "true");
    expect(screen.getByRole("checkbox", { name: "Locked" })).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByRole("checkbox", { name: "Read only" })).toHaveAttribute("aria-readonly", "true");
  });

  it("groups checkboxes with a parent that checks them all", async () => {
    const onChange = vi.fn();
    render(
      <CheckboxGroup aria-label="Rights" defaultValue={["read"]} allValues={["read", "write"]} onValueChange={onChange}>
        <Checkbox.Box parent aria-label="All rights" />
        <Checkbox value="read" label="Read" />
        <Checkbox value="write" label="Write" />
      </CheckboxGroup>,
    );
    await userEvent.click(screen.getByRole("checkbox", { name: "All rights" }));
    expect(onChange).toHaveBeenLastCalledWith(["read", "write"]);
    render(
      <CheckboxGroup aria-label="Locked" value={[]} disabled>
        <Checkbox value="a" label="A" />
      </CheckboxGroup>,
    );
    expect(screen.getByRole("checkbox", { name: "A" })).toHaveAttribute("aria-disabled", "true");
  });
});

describe("Collapsible and Accordion", () => {
  it("opens and closes, controlled or not", async () => {
    const onOpenChange = vi.fn();
    render(
      <>
        <Collapsible title="Advanced" hint="3 settings" onOpenChange={onOpenChange}>
          <p>Retries</p>
        </Collapsible>
        <Collapsible title="Shown" open>
          <p>Visible</p>
        </Collapsible>
      </>,
    );
    await userEvent.click(screen.getByRole("button", { name: /Advanced/ }));
    expect(onOpenChange).toHaveBeenCalledWith(true);
    expect(screen.getByText("Visible")).toBeInTheDocument();
  });

  it("opens several panels when multiple", async () => {
    render(
      <Accordion
        multiple
        defaultValue={["a"]}
        items={[
          { value: "a", title: "Billing", content: <p>Invoices</p>, hint: "Monthly" },
          { value: "b", title: "Limits", content: <p>Budgets</p> },
        ]}
      />,
    );
    expect(screen.getByText("Invoices")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Limits" }));
    expect(screen.getByText("Budgets")).toBeInTheDocument();
    expect(screen.getByText("Invoices")).toBeInTheDocument();
  });
});

describe("SensitiveInput", () => {
  it("reveals and hides a secret in mono", async () => {
    render(<SensitiveInput aria-label="API key" defaultValue="sk-kaeso-live" />);
    const input = screen.getByLabelText("API key");
    expect(input).toHaveAttribute("type", "password");
    expect(input).toHaveAttribute("data-1p-ignore");
    await userEvent.click(screen.getByRole("button", { name: "Show" }));
    expect(input).toHaveAttribute("type", "text");
    await userEvent.click(screen.getByRole("button", { name: "Hide" }));
    expect(input).toHaveAttribute("type", "password");
  });

  it("warns about Caps Lock in a password", async () => {
    const onKeyDown = vi.fn();
    const onKeyUp = vi.fn();
    render(<SensitiveInput kind="password" label="Password" description="At least 12 characters" onKeyDown={onKeyDown} onKeyUp={onKeyUp} />);
    const input = screen.getByLabelText("Password");
    expect(input).toHaveAttribute("autocomplete", "current-password");
    await userEvent.type(input, "{CapsLock}a");
    expect(screen.getByText("Caps Lock is on.")).toBeInTheDocument();
    expect(onKeyDown).toHaveBeenCalled();
    expect(onKeyUp).toHaveBeenCalled();
  });
});

describe("Popover and HoverCard", () => {
  it("opens a popover named by its title", async () => {
    render(
      <Popover>
        <Popover.Trigger>
          <button>Filters</button>
        </Popover.Trigger>
        <Popover.Content title="Filter requests">
          <p>Status</p>
        </Popover.Content>
      </Popover>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Filters" }));
    expect(await screen.findByRole("dialog", { name: "Filter requests" })).toHaveTextContent("Status");
  });

  it("names an untitled popover with its label", async () => {
    render(
      <Popover open>
        <Popover.Trigger>
          <button>Columns</button>
        </Popover.Trigger>
        <Popover.Content aria-label="Visible columns">
          <p>Model</p>
        </Popover.Content>
      </Popover>,
    );
    expect(await screen.findByRole("dialog", { name: "Visible columns" })).toBeInTheDocument();
  });

  it("opens a hover card when controlled", async () => {
    const onOpenChange = vi.fn();
    render(
      <HoverCard open onOpenChange={onOpenChange}>
        <HoverCard.Trigger delay={0}>
          <a href="#m">claude-sonnet-4-5</a>
        </HoverCard.Trigger>
        <HoverCard.Content side="top" align="center">
          <p>$3.00 / M input</p>
        </HoverCard.Content>
      </HoverCard>,
    );
    expect(await screen.findByText("$3.00 / M input")).toBeInTheDocument();
    render(
      <HoverCard defaultOpen>
        <HoverCard.Trigger>
          <a href="#n">gpt-5</a>
        </HoverCard.Trigger>
        <HoverCard.Content>
          <p>Default open</p>
        </HoverCard.Content>
      </HoverCard>,
    );
    expect(await screen.findByText("Default open")).toBeInTheDocument();
  });
});

describe("focusInputOnPress", () => {
  it("focuses the input when its frame is pressed, not when the press is inside", () => {
    render(
      <div data-testid="frame" onMouseDown={focusInputOnPress}>
        <span data-testid="inside">icon</span>
        <input aria-label="Search" />
      </div>,
    );
    const frame = screen.getByTestId("frame");
    screen.getByTestId("inside").dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    expect(screen.getByLabelText("Search")).not.toHaveFocus();
    frame.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    expect(screen.getByLabelText("Search")).toHaveFocus();
  });

  it("does nothing without an enabled input", () => {
    render(
      <>
        <div data-testid="empty" onMouseDown={focusInputOnPress} />
        <div data-testid="disabled" onMouseDown={focusInputOnPress}>
          <input aria-label="Off" disabled />
        </div>
      </>,
    );
    screen.getByTestId("empty").dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    screen.getByTestId("disabled").dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    expect(screen.getByLabelText("Off")).not.toHaveFocus();
  });
});
