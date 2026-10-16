import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import { LinkProvider, ThemeProvider } from "@logicplanes/kaeso-ui";
import { createDocsRouter } from "~/router";
import { RouterLink } from "~/shell/router-link";
import { AppToaster } from "~/shell/toaster";
import * as load from "./load";

vi.mock("./load", async (original) => {
  const real = await original<typeof load>();
  return { ...real, loadPage: vi.fn(real.loadPage) };
});

describe("a page that can't load", () => {
  it("keeps the frame, says so in a toast, and loads on Try again", async () => {
    vi.mocked(load.loadPage).mockRejectedValueOnce(new Error("offline"));
    const user = userEvent.setup();
    render(
      <ThemeProvider>
        <LinkProvider component={RouterLink}>
          <RouterProvider router={createDocsRouter(createMemoryHistory({ initialEntries: ["/guides/provider-keys"] }))} />
        </LinkProvider>
        <AppToaster />
      </ThemeProvider>,
    );
    expect(await screen.findByText("This page couldn't load")).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "Documentation" })).toBeInTheDocument();
    expect((await screen.findAllByText("This page couldn't load; check the connection and try again", {}, { timeout: 5000 }))[0]).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Provider keys" })).toBeInTheDocument();
  });
});
