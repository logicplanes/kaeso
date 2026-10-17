import { afterEach, describe, expect, it } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import { LinkProvider, ThemeProvider } from "@logicplanes/kaeso-ui";
import { LOCALE_STORAGE_KEY } from "~/i18n/locales";
import { createDocsRouter } from "~/router";
import { RouterLink } from "~/shell/router-link";

afterEach(() => localStorage.clear());

function phoneBar() {
  const bar = document.querySelector<HTMLElement>("[data-phone-bar]");
  if (!bar) throw new Error("the page has no phone bar");
  return within(bar);
}

function app(path: string) {
  const history = createMemoryHistory({ initialEntries: [path] });
  const router = createDocsRouter(history);
  render(
    <ThemeProvider>
      <LinkProvider component={RouterLink}>
        <RouterProvider router={router} />
      </LinkProvider>
    </ThemeProvider>,
  );
  return router;
}

describe("the site", () => {
  it("opens a page with its title, description, sidebar and footer", async () => {
    app("/guides/keys-and-limits");
    expect(await screen.findByRole("heading", { level: 1, name: "Keys and limits" })).toBeInTheDocument();
    expect(screen.getByText("Create a key for an app and keep its spending inside a budget.")).toBeInTheDocument();
    const nav = screen.getByRole("navigation", { name: "Documentation" });
    expect(within(nav).getByRole("link", { name: "Keys and limits" })).toHaveAttribute("aria-current", "page");
    expect(screen.getAllByRole("link", { current: "page" }).map((link) => link.textContent)).toEqual(["Keys and limits"]);
    expect(screen.getByRole("link", { name: /Edit on GitHub/ })).toHaveAttribute(
      "href",
      "https://github.com/logicplanes/kaeso/edit/dev/apps/docs/e2e/fixtures/content/en/guides/keys-and-limits.md",
    );
    const steps = screen.getByRole("navigation", { name: "Previous and next page" });
    expect(within(steps).getByRole("link", { name: "Next: Provider keys" })).toHaveAttribute("href", "/guides/provider-keys");
    await waitFor(() => expect(document.title).toBe("Keys and limits · Kaeso documentation"));
  });

  it("opens the same page in another language from the language select, and remembers the choice", async () => {
    const user = userEvent.setup();
    const router = app("/guides/keys-and-limits");
    await screen.findByRole("heading", { level: 1, name: "Keys and limits" });
    const language = screen.getByRole("combobox", { name: "Language" });
    expect(language).toHaveTextContent("English");
    await user.click(language);
    expect((await screen.findAllByRole("option")).map((o) => o.textContent)).toEqual(["English", "Deutsch", "简体中文"]);
    await user.click(screen.getByRole("option", { name: "Deutsch" }));
    await waitFor(() => expect(router.state.location.pathname).toBe("/de/guides/keys-and-limits"));
    expect(await screen.findByRole("heading", { level: 1, name: "Schlüssel und Limits" })).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "Dokumentation" })).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Sprache" })).toHaveTextContent("Deutsch");
    expect(screen.getByRole("combobox", { name: "Design" })).toHaveTextContent("System");
    expect(localStorage.getItem(LOCALE_STORAGE_KEY)).toBe("de");
    await waitFor(() => expect(document.documentElement.lang).toBe("de"));
  });

  it("shows the Chinese pages with their own text", async () => {
    app("/zh-CN");
    expect(await screen.findByRole("navigation", { name: "文档" })).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "语言" })).toHaveTextContent("简体中文");
    expect(screen.getByRole("combobox", { name: "主题" })).toHaveTextContent("跟随系统");
  });

  it("says when an address has no page, in the frame and in its language", async () => {
    app("/de/nothing-here");
    expect(await screen.findByText("Diese Seite gibt es nicht.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Zur Startseite" })).toHaveAttribute("href", "/de");
  });
});

describe("previous and next", () => {
  for (const [path, previous, next] of [
    ["/guides/keys-and-limits", ["Previous", "Guides", "Previous: Guides"], ["Next", "Provider keys", "Next: Provider keys"]],
    ["/de/guides/keys-and-limits", ["Zurück", "Anleitungen", "Zurück: Anleitungen"], ["Weiter", "Anbieter-Schlüssel", "Weiter: Anbieter-Schlüssel"]],
    ["/zh-CN/guides/keys-and-limits", ["上一页", "指南", "上一页：指南"], ["下一页", "服务商密钥", "下一页：服务商密钥"]],
  ] as const) {
    it(`shows the label above the title and names the link after both, at ${path}`, async () => {
      app(path);
      const steps = await screen.findByRole("navigation", { name: /Previous and next page|Vorherige und nächste Seite|上一页和下一页/ });
      for (const [label, title, name] of [previous, next]) {
        const link = within(steps).getByRole("link", { name });
        expect(link).toHaveClass("flex", "flex-col");
        expect([...link.children].map((child) => child.textContent)).toEqual([label, title]);
      }
      expect(within(steps).getByRole("link", { name: next[2] })).toHaveClass("items-end", "text-right");
      expect(within(steps).getByRole("link", { name: previous[2] })).toHaveClass("items-start");
    });
  }
});

describe("search", () => {
  it("finds a heading and opens its page there", async () => {
    const user = userEvent.setup();
    const router = app("/");
    await screen.findByRole("heading", { level: 1, name: "Kaeso documentation" });
    await user.click(within(screen.getByRole("navigation", { name: "Documentation" })).getByRole("button", { name: "Search" }));
    const input = await screen.findByPlaceholderText("Search the documentation");
    await user.type(input, "rate");
    await user.click(await screen.findByRole("option", { name: /Rate limits/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe("/guides/keys-and-limits"));
    expect(router.state.location.hash).toBe("rate-limits");
  });

  it("opens from the phone bar, beside the navigation toggle, in the page's language", async () => {
    const user = userEvent.setup();
    app("/de/guides");
    await screen.findByRole("heading", { level: 1, name: "Anleitungen" });
    const bar = phoneBar();
    expect(bar.getAllByRole("button").map((b) => b.getAttribute("aria-label"))).toEqual(["Suchen", "Navigation öffnen"]);
    expect(bar.getByRole("button", { name: "Suchen" })).toHaveAttribute("aria-haspopup", "dialog");
    await user.click(bar.getByRole("button", { name: "Suchen" }));
    expect(await screen.findByPlaceholderText("Dokumentation durchsuchen")).toHaveFocus();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByPlaceholderText("Dokumentation durchsuchen")).toBeNull());
  });

  it("switches the language from the search", async () => {
    const user = userEvent.setup();
    const router = app("/reference/models");
    await screen.findByRole("heading", { level: 1, name: "Models" });
    await user.keyboard("{Control>}k{/Control}");
    await user.type(await screen.findByPlaceholderText("Search the documentation"), "简体");
    await user.click(await screen.findByRole("option", { name: /简体中文/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe("/zh-CN/reference/models"));
  });

  it("changes the theme from the search", async () => {
    const user = userEvent.setup();
    app("/");
    await screen.findByRole("heading", { level: 1, name: "Kaeso documentation" });
    await user.click(phoneBar().getByRole("button", { name: "Search" }));
    await user.type(await screen.findByPlaceholderText("Search the documentation"), "dark");
    await user.click(await screen.findByRole("option", { name: "Use the dark theme" }));
    await waitFor(() => expect(document.documentElement).toHaveAttribute("data-theme", "dark"));
    expect(screen.getByRole("combobox", { name: "Theme" })).toHaveTextContent("Dark");
  });
});
