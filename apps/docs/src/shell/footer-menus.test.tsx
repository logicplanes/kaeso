import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import { LinkProvider, ThemeProvider } from "@logicplanes/kaeso-ui";
import { createDocsRouter } from "~/router";
import { RouterLink } from "./router-link";

const footerMenus = vi.hoisted(() => {
  let release: () => void = () => undefined;
  const hold = new Promise<void>((resolve) => (release = resolve));
  return { hold, release: () => release() };
});

vi.mock("./footer-menus", async (importOriginal) => {
  await footerMenus.hold;
  return importOriginal();
});

describe("the foot of the sidebar", () => {
  const renderDocs = () =>
    render(
      <ThemeProvider>
        <LinkProvider component={RouterLink}>
          <RouterProvider router={createDocsRouter(createMemoryHistory({ initialEntries: ["/"] }))} />
        </LinkProvider>
      </ThemeProvider>,
    );

  it("shows the three themes side by side and the current language by name, and opens the language clicked while the menu was loading", async () => {
    const user = userEvent.setup();
    renderDocs();
    await screen.findByRole("heading", { level: 1, name: "Kaeso documentation" });
    const theme = screen.getByRole("radiogroup", { name: "Theme" });
    expect(theme).toBeInTheDocument();
    expect(screen.getAllByRole("radio").map((r) => r.getAttribute("aria-label"))).toEqual(["System", "Light", "Dark"]);
    const language = screen.getByRole("button", { name: "Language: English" });
    expect(language).toHaveTextContent("English");
    await user.click(language);
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    footerMenus.release();
    expect((await screen.findAllByRole("menuitemradio")).map((o) => o.textContent)).toEqual(["English", "Deutsch", "简体中文"]);
  });

  it("switches the theme with one tap", async () => {
    const user = userEvent.setup();
    footerMenus.release();
    renderDocs();
    await screen.findByRole("heading", { level: 1, name: "Kaeso documentation" });
    await user.click(screen.getByRole("radio", { name: "Dark" }));
    expect(document.documentElement).toHaveAttribute("data-theme", "dark");
  });
});
