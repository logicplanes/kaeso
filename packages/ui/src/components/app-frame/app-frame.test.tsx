import { beforeEach, describe, expect, it } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AppFrame } from "./app-frame";
import { PageHeader } from "../page-header/page-header";
import { Sidebar } from "../sidebar/sidebar";
import { Button } from "../button/button";

function Page(props: { description?: string }) {
  return (
    <Sidebar.Provider shortcut={false}>
      <AppFrame.Settings title="Members" actions={<Button size="sm">Invite member</Button>} {...props}>
        <p>Rows</p>
      </AppFrame.Settings>
    </Sidebar.Provider>
  );
}

describe("AppFrame.Settings", () => {
  it("puts the actions on the title's line and the description below it", () => {
    render(<Page description="People in the organization and their rights in it." />);
    const line = screen.getByRole("heading", { level: 1, name: "Members" }).parentElement!;
    expect(line).toContainElement(screen.getByRole("button", { name: "Invite member" }));
    const description = screen.getByText("People in the organization and their rights in it.");
    expect(line).not.toContainElement(description);
    expect(line.compareDocumentPosition(description) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });
  it("keeps the actions on the title's line without a description", () => {
    render(<Page />);
    const line = screen.getByRole("heading", { level: 1, name: "Members" }).parentElement!;
    expect(line).toContainElement(screen.getByRole("button", { name: "Invite member" }));
  });
});

function Shell({ brandHref }: { brandHref?: string }) {
  return (
    <Sidebar.Provider shortcut={false}>
      <AppFrame
        brandHref={brandHref}
        brandLabel="Kaeso home"
        sidebar={
          <Sidebar aria-label="Console">
            <Sidebar.Brand href="/" label="Kaeso home">
              <Sidebar.CollapseToggle />
            </Sidebar.Brand>
            <Sidebar.Content>
              <Sidebar.Group>
                <Sidebar.Item href="/logs">Request logs</Sidebar.Item>
              </Sidebar.Group>
            </Sidebar.Content>
          </Sidebar>
        }
      >
        <PageHeader start={<AppFrame.MenuButton />} title="Request logs" />
      </AppFrame>
    </Sidebar.Provider>
  );
}

describe("AppFrame's phone bar", () => {
  beforeEach(() =>
    (window as unknown as { happyDOM: { setViewport: (v: { width: number; height: number }) => void } }).happyDOM.setViewport({ width: 390, height: 844 }),
  );
  it("puts the wordmark alone first and the navigation toggle after it", () => {
    render(<Shell brandHref="/" />);
    const bar = document.querySelector<HTMLElement>("[data-phone-bar]")!;
    expect(bar.tagName).toBe("HEADER");
    const link = within(bar).getByRole("link", { name: "Kaeso home" });
    const toggle = within(bar).getByRole("button", { name: "Open navigation" });
    expect(link.compareDocumentPosition(toggle) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(within(bar).getAllByRole("link")).toHaveLength(1);
    expect(within(bar).getAllByRole("button")).toHaveLength(1);
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(toggle).toHaveAttribute("aria-haspopup", "dialog");
    expect(bar.compareDocumentPosition(screen.getByRole("main")) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });
  it("shows the plain wordmark, named, without a home link", () => {
    render(<Shell />);
    const bar = document.querySelector<HTMLElement>("[data-phone-bar]")!;
    expect(within(bar).queryByRole("link")).toBeNull();
    expect(within(bar).getByRole("img", { name: "Kaeso home" })).toBeInTheDocument();
  });
  it("opens the sidebar sheet, says so, and gives the focus back to the toggle when it closes", async () => {
    render(<Shell brandHref="/" />);
    const toggle = within(document.querySelector<HTMLElement>("[data-phone-bar]")!).getByRole("button", { name: "Open navigation" });
    toggle.focus();
    await userEvent.keyboard("{Enter}");
    expect(await screen.findByRole("dialog", { name: "Navigation" })).toBeInTheDocument();
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(toggle).toHaveAttribute("aria-expanded", "false"));
    await waitFor(() => expect(toggle).toHaveFocus());
  });
  it("closes the sheet from the sidebar's own toggle", async () => {
    render(<Shell brandHref="/" />);
    const toggle = within(document.querySelector<HTMLElement>("[data-phone-bar]")!).getByRole("button", { name: "Open navigation" });
    await userEvent.click(toggle);
    const sheet = await screen.findByRole("dialog", { name: "Navigation" });
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    await userEvent.click(within(sheet).getByRole("button", { name: "Close navigation" }));
    await waitFor(() => expect(toggle).toHaveAttribute("aria-expanded", "false"));
  });
  it("leaves the page header's menu button to wide screens with a collapsed sidebar", () => {
    render(<Shell />);
    const header = screen.getByRole("heading", { level: 1, name: "Request logs" }).closest("header")!;
    const menu = within(header).getByRole("button", { hidden: true });
    expect(menu).toHaveClass("hidden");
    expect(menu).not.toHaveClass("lg:inline-flex");
  });
});
