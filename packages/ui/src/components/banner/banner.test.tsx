import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Banner, KAESO_BANNER_VARIANTS } from "./banner.js";
import { Button } from "../button/button.js";
import { PageHeader } from "../page-header/page-header.js";
import { KAESO_APP_FRAME_BODY_VARIANTS } from "../app-frame/app-frame.js";

function Deletion({ loading = false }: { loading?: boolean }) {
  return (
    <Banner
      layout="full"
      tone="danger"
      title="This organization is scheduled for deletion"
      actions={
        <Button size="sm" loading={loading}>
          Restore organization
        </Button>
      }
    >
      Its keys stopped working; on 20 Oct 2026 it is deleted for good.
    </Banner>
  );
}

describe("Banner", () => {
  it("is an alert when it is about danger, a status otherwise", () => {
    const { rerender } = render(<Deletion />);
    expect(screen.getByRole("alert")).toHaveTextContent("This organization is scheduled for deletion");
    rerender(<Banner tone="warning">This key expires in 5 days.</Banner>);
    expect(screen.getByRole("status")).toHaveTextContent("This key expires in 5 days.");
  });
  it("is its own size container: the actions go beside the text only from the banner width", () => {
    render(<Deletion />);
    const banner = screen.getByRole("alert");
    expect(banner).toHaveClass("@container", "w-full");
    const actions = screen.getByRole("button", { name: "Restore organization" }).parentElement!;
    const main = actions.parentElement!;
    expect(main).toHaveClass("flex-col", "@banner:flex-row", "@banner:items-center");
    expect(actions).toHaveClass("ml-24", "@banner:ml-0", "shrink-0");
    const text = screen.getByText("This organization is scheduled for deletion").parentElement!;
    expect(text).toHaveTextContent("Its keys stopped working");
    expect(text).not.toContainElement(actions);
    expect(text.parentElement!.firstElementChild!.tagName.toLowerCase()).toBe("svg");
  });
  it("uses no negative margins to place the actions", () => {
    render(<Deletion />);
    const actions = screen.getByRole("button", { name: "Restore organization" }).parentElement!;
    expect(actions.className).not.toMatch(/(^|\s)-m[ytb]?-/);
  });
  it("keeps every class when an action starts loading", () => {
    const { rerender, container } = render(<Deletion />);
    const before = [...container.querySelectorAll("*")].filter((el) => !el.closest("button")).map((el) => el.getAttribute("class"));
    rerender(<Deletion loading />);
    const after = [...container.querySelectorAll("*")].filter((el) => !el.closest("button")).map((el) => el.getAttribute("class"));
    expect(after).toEqual(before);
    expect(screen.getByRole("button", { name: "Restore organization" })).toHaveAttribute("aria-busy", "true");
  });
  it("full width shares the page's 24px side inset with the page header and a content page", () => {
    render(<PageHeader title="Request logs" />);
    const header = screen.getByRole("banner");
    expect(KAESO_BANNER_VARIANTS.layout.full.classes).toContain("px-24");
    expect(header).toHaveClass("px-24");
    expect(KAESO_APP_FRAME_BODY_VARIANTS.padding.page.classes).toContain("px-24");
  });
  it("can be dismissed only when it informs", async () => {
    const onDismiss = vi.fn();
    const { rerender } = render(
      <Banner tone="info" title="New models are available" actions={<Button size="sm">Review</Button>} onDismiss={onDismiss}>
        KPMR added 4 models since the last sync.
      </Banner>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(onDismiss).toHaveBeenCalledOnce();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    rerender(<Deletion />);
    expect(screen.queryByRole("button", { name: "Dismiss" })).not.toBeInTheDocument();
  });
});

describe("Banner, details", () => {
  it("warns when a banner that can't be dismissed is given onDismiss, and shows no dismiss button", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    render(
      <Banner tone="danger" onDismiss={() => undefined}>
        The provider key was revoked.
      </Banner>,
    );
    expect(warn).toHaveBeenCalledWith(expect.stringContaining("can't be dismissed"));
    expect(screen.queryByRole("button", { name: "Dismiss" })).toBeNull();
    warn.mockRestore();
  });
  it("tells neutral from information by more than the tint: only information carries the info icon", () => {
    const { container, rerender } = render(<Banner>Maintenance is planned for Sunday.</Banner>);
    expect(container.querySelector("svg")).toBeNull();
    rerender(<Banner tone="info">New models are available.</Banner>);
    expect(container.querySelector("svg")).not.toBeNull();
  });
  it("keeps one height whether or not it has an action, and an outline in forced colours", () => {
    render(<Banner>Maintenance is planned for Sunday.</Banner>);
    expect(screen.getByRole("status")).toHaveClass("min-h-56", "forced-colors:outline");
  });
});
