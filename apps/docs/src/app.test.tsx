import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import { LinkProvider, ThemeProvider } from "@logicplanes/kaeso-ui";
import { LOCALE_STORAGE_KEY } from "~/i18n/locales";
import { createDocsRouter } from "~/router";
import { RouterLink } from "~/shell/router-link";
import { setSearchOpen } from "~/shell/search-store";

const searchIndex = vi.hoisted(() => ({ hold: null as Promise<void> | null }));

vi.mock("~/site/data", async (importOriginal) => {
  const actual = await importOriginal<typeof import("~/site/data")>();
  return {
    ...actual,
    loadSearchIndex: async (locale: Parameters<typeof actual.loadSearchIndex>[0]) => {
      await searchIndex.hold;
      return actual.loadSearchIndex(locale);
    },
  };
});

afterEach(() => {
  localStorage.clear();
  searchIndex.hold = null;
  setSearchOpen(false);
});

function holdSearch() {
  let release: () => void = () => undefined;
  searchIndex.hold = new Promise<void>((resolve) => (release = resolve));
  return release;
}

function phoneBar() {
  const bar = document.querySelector<HTMLElement>("[data-phone-bar]");
  if (!bar) throw new Error("the page has no phone bar");
  return within(bar);
}

async function themeChoice(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "Theme" }));
  const checked = (await screen.findAllByRole("menuitemradio")).find((o) => o.getAttribute("aria-checked") === "true");
  await user.keyboard("{Escape}");
  return checked?.textContent;
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
    const user = userEvent.setup();
    app("/guides/keys-and-limits");
    expect(await screen.findByRole("heading", { level: 1, name: "Keys and limits" })).toBeInTheDocument();
    expect(screen.getByText("Create a key for an app and keep its spending inside a budget.")).toBeInTheDocument();
    const nav = screen.getByRole("navigation", { name: "Documentation" });
    expect(within(nav).getByRole("link", { name: "Keys and limits" })).toHaveAttribute("aria-current", "page");
    expect(screen.getAllByRole("link", { current: "page" }).map((link) => link.textContent)).toEqual(["Keys and limits"]);
    expect(within(nav).getByRole("link", { name: "Changelog" })).toHaveAttribute("href", "https://kaeso.app/changelog");
    await user.click(within(nav).getByRole("button", { name: "Legal" }));
    expect(await screen.findByRole("menuitem", { name: "Privacy" })).toHaveAttribute("href", "https://kaeso.app/datenschutz");
    expect(screen.getByRole("menuitem", { name: "Imprint" })).toHaveAttribute("href", "https://kaeso.app/impressum");
    await user.keyboard("{Escape}");
    expect(screen.getByRole("link", { name: /Edit on GitHub/ })).toHaveAttribute(
      "href",
      "https://github.com/logicplanes/kaeso/edit/dev/apps/docs/e2e/fixtures/content/en/guides/keys-and-limits.md",
    );
    const steps = screen.getByRole("navigation", { name: "Previous and next page" });
    expect(within(steps).getByRole("link", { name: "Next: Provider keys" })).toHaveAttribute("href", "/guides/provider-keys");
    await waitFor(() => expect(document.title).toBe("Keys and limits · Kaeso documentation"));
  });

  it("opens the same page in another language from the language menu, and remembers the choice", async () => {
    const user = userEvent.setup();
    const router = app("/guides/keys-and-limits");
    await screen.findByRole("heading", { level: 1, name: "Keys and limits" });
    await user.click(screen.getByRole("button", { name: "Language" }));
    expect((await screen.findAllByRole("menuitemradio")).map((o) => o.textContent)).toEqual(["English", "Deutsch", "简体中文"]);
    expect(screen.getByRole("menuitemradio", { name: "English" })).toHaveAttribute("aria-checked", "true");
    await user.click(screen.getByRole("menuitemradio", { name: "Deutsch" }));
    await waitFor(() => expect(router.state.location.pathname).toBe("/de/guides/keys-and-limits"));
    expect(await screen.findByRole("heading", { level: 1, name: "Schlüssel und Limits" })).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "Dokumentation" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Rechtliches" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Design" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Sprache" }));
    expect(await screen.findByRole("menuitemradio", { name: "Deutsch" })).toHaveAttribute("aria-checked", "true");
    await user.keyboard("{Escape}");
    expect(localStorage.getItem(LOCALE_STORAGE_KEY)).toBe("de");
    await waitFor(() => expect(document.documentElement.lang).toBe("de"));
  });

  it("shows the Chinese pages with their own text", async () => {
    app("/zh-CN");
    expect(await screen.findByRole("navigation", { name: "文档" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "法律信息" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "主题" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "语言" })).toBeInTheDocument();
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

  it("opens with an empty query each time, without the last search's results", async () => {
    const user = userEvent.setup();
    app("/");
    await screen.findByRole("heading", { level: 1, name: "Kaeso documentation" });
    await user.keyboard("{Control>}k{/Control}");
    await user.type(await screen.findByPlaceholderText("Search the documentation"), "rate");
    expect(await screen.findByRole("option", { name: /Rate limits/ })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByPlaceholderText("Search the documentation")).toBeNull());
    await user.keyboard("{Control>}k{/Control}");
    expect(await screen.findByPlaceholderText("Search the documentation")).toHaveValue("");
    expect(await screen.findByRole("option", { name: /^Kaeso documentation/ })).toBeInTheDocument();
    expect(screen.queryByRole("option", { name: /Rate limits/ })).toBeNull();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByPlaceholderText("Search the documentation")).toBeNull());
  });

  it("keeps everything typed after ⌘K while the search is still loading", async () => {
    const user = userEvent.setup();
    const release = holdSearch();
    app("/");
    await screen.findByRole("heading", { level: 1, name: "Kaeso documentation" });
    await user.keyboard("{Control>}k{/Control}");
    await user.keyboard("rat");
    expect(screen.queryByPlaceholderText("Search the documentation")).toBeNull();
    await user.keyboard("x{Backspace}");
    release();
    const input = await screen.findByPlaceholderText("Search the documentation");
    expect(input).toHaveValue("rat");
    await user.keyboard("e");
    expect(input).toHaveValue("rate");
    expect(await screen.findByRole("option", { name: /Rate limits/ })).toBeInTheDocument();
  });

  it("keeps what is typed in the moment between the search appearing and its field taking focus", async () => {
    const user = userEvent.setup();
    app("/");
    await screen.findByRole("heading", { level: 1, name: "Kaeso documentation" });
    await user.keyboard("{Control>}k{/Control}");
    const input = await screen.findByPlaceholderText("Search the documentation");
    input.blur();
    expect(input).not.toHaveFocus();
    await user.keyboard("ali");
    expect(input).toHaveValue("ali");
    input.focus();
    await user.keyboard("as");
    expect(input).toHaveValue("alias");
    expect(await screen.findByRole("option", { name: /^Aliases/ })).toBeInTheDocument();
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
    expect(await themeChoice(user)).toBe("Dark");
    for (const [query, option, label] of [
      ["light", "Use the light theme", "Light"],
      ["system", "Use the system theme", "System"],
    ] as const) {
      await user.click(phoneBar().getByRole("button", { name: "Search" }));
      await user.type(await screen.findByPlaceholderText("Search the documentation"), query);
      await user.click(await screen.findByRole("option", { name: option }));
      expect(await themeChoice(user)).toBe(label);
    }
  });
});
