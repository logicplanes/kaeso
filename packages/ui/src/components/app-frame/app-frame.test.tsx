import type { ReactNode } from "react";
import { beforeEach, describe, expect, it } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AppFrame } from "./app-frame.js";
import { PageHeader } from "../page-header/page-header.js";
import { Sidebar } from "../sidebar/sidebar.js";
import { Button } from "../button/button.js";
import { IconButton } from "../icon-button/icon-button.js";
import { icons } from "../../icons/vocabulary.js";
import { LocaleProvider, type UiMessages } from "../../i18n/index.js";
import de from "../../i18n/locales/de.js";
import zhCN from "../../i18n/locales/zh-CN.js";

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

function Shell({ brandHref, phoneActions }: { brandHref?: string; phoneActions?: ReactNode }) {
  return (
    <Sidebar.Provider shortcut={false}>
      <AppFrame
        brandHref={brandHref}
        brandLabel="Kaeso home"
        phoneActions={phoneActions}
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
  it("puts its own actions between the wordmark and the navigation toggle", async () => {
    let searched = 0;
    render(<Shell brandHref="/" phoneActions={<IconButton icon={icons.search} aria-label="Search" tooltip={false} onClick={() => (searched += 1)} />} />);
    const bar = document.querySelector<HTMLElement>("[data-phone-bar]")!;
    expect(
      within(bar)
        .getAllByRole("button")
        .map((b) => b.getAttribute("aria-label")),
    ).toEqual(["Search", "Open navigation"]);
    await userEvent.click(within(bar).getByRole("button", { name: "Search" }));
    expect(searched).toBe(1);
    expect(within(bar).getByRole("button", { name: "Open navigation" })).toHaveAttribute("aria-expanded", "false");
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

function Console({ locale = "en", messages = {} }: { locale?: string; messages?: UiMessages }) {
  return (
    <LocaleProvider locale={locale} messages={messages}>
      <Sidebar.Provider shortcut={false}>
        <AppFrame
          brandHref="/"
          brandLabel="Kaeso home"
          sidebar={
            <Sidebar aria-label="Console">
              <Sidebar.Content>
                <Sidebar.Group>
                  <Sidebar.Item href="/logs">Request logs</Sidebar.Item>
                  <Sidebar.Item href="/keys">Keys</Sidebar.Item>
                </Sidebar.Group>
              </Sidebar.Content>
            </Sidebar>
          }
        >
          <PageHeader title="Playground" />
          <AppFrame.Body>
            <textarea aria-label="Prompt" />
          </AppFrame.Body>
        </AppFrame>
      </Sidebar.Provider>
    </LocaleProvider>
  );
}

describe("AppFrame's skip link", () => {
  it("is the first stop, and moves the focus past the navigation to the content", async () => {
    const user = userEvent.setup();
    render(<Console />);
    await user.tab();
    const skip = screen.getByRole("link", { name: "Skip to content" });
    expect(skip).toHaveFocus();
    expect(skip).toHaveClass("not-focus:sr-only");
    await user.keyboard("{Enter}");
    const main = screen.getByRole("main");
    expect(main).toHaveFocus();
    expect(skip).toHaveAttribute("href", `#${main.id}`);
    await user.tab();
    expect(screen.getByRole("textbox", { name: "Prompt" })).toHaveFocus();
    expect(main).not.toHaveAttribute("tabindex");
  });
  it("leaves the main region out of the tab order and click focus until it is used", async () => {
    render(<Console />);
    await userEvent.click(screen.getByRole("heading", { name: "Playground" }));
    expect(screen.getByRole("main")).not.toHaveFocus();
    expect(screen.getByRole("main")).not.toHaveAttribute("tabindex");
  });
  it.each<[string, UiMessages, string]>([
    ["de", de, "Zum Inhalt springen"],
    ["zh-CN", zhCN, "跳到主要内容"],
  ])("%s: says so in the page's language", (locale, messages, name) => {
    render(<Console locale={locale} messages={messages} />);
    expect(screen.getByRole("link", { name })).toBeInTheDocument();
  });
  it("gives every frame on a page its own target", () => {
    render(
      <>
        <Console />
        <Console />
      </>,
    );
    const [a, b] = screen.getAllByRole("link", { name: "Skip to content" }).map((l) => l.getAttribute("href"));
    expect(a).not.toBe(b);
    expect(document.querySelector(a!)).toBeInstanceOf(HTMLElement);
  });
});
