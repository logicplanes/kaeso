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

describe("the menus at the foot of the sidebar", () => {
  it("open the menu clicked while they were still loading once they have loaded", async () => {
    const user = userEvent.setup();
    render(
      <ThemeProvider>
        <LinkProvider component={RouterLink}>
          <RouterProvider router={createDocsRouter(createMemoryHistory({ initialEntries: ["/"] }))} />
        </LinkProvider>
      </ThemeProvider>,
    );
    await screen.findByRole("heading", { level: 1, name: "Kaeso documentation" });
    for (const name of ["Legal", "Theme", "Language"]) await user.click(screen.getByRole("button", { name }));
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    footerMenus.release();
    expect((await screen.findAllByRole("menuitemradio")).map((o) => o.textContent)).toEqual(["English", "Deutsch", "简体中文"]);
  });
});
