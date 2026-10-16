import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import { LinkProvider, ThemeProvider } from "@logicplanes/kaeso-ui";
import { createDocsRouter } from "~/router";
import { RouterLink } from "~/shell/router-link";
import * as data from "~/site/data";

vi.mock("~/site/data", async (original) => {
  const real = await original<typeof data>();
  return { ...real, loadLocaleData: vi.fn(real.loadLocaleData) };
});

describe("a language that can't load", () => {
  it("says so in place of the frame and loads on Try again", async () => {
    vi.mocked(data.loadLocaleData).mockRejectedValueOnce(new Error("offline"));
    const user = userEvent.setup();
    render(
      <ThemeProvider>
        <LinkProvider component={RouterLink}>
          <RouterProvider router={createDocsRouter(createMemoryHistory({ initialEntries: ["/de"] }))} />
        </LinkProvider>
      </ThemeProvider>,
    );
    expect(await screen.findByText("This page couldn't load")).toBeInTheDocument();
    expect(screen.queryByRole("navigation", { name: "Dokumentation" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("navigation", { name: "Dokumentation" })).toBeInTheDocument();
  });
});
