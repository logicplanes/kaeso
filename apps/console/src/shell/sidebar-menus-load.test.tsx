import { act, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ORG, PROJECT, signedIn } from "~/test/fixtures";
import { on, page } from "~/test/network";
import { renderApp } from "~/test/render";

const menus = vi.hoisted(() => {
  let release = () => {};
  const ready = new Promise<void>((resolve) => (release = resolve));
  return { attempts: 0, ready, release: () => release() };
});

vi.mock("~/shell/sidebar-menus", async (importOriginal) => {
  menus.attempts += 1;
  if (menus.attempts === 1) throw new TypeError("Failed to fetch dynamically imported module");
  await menus.ready;
  return importOriginal();
});

describe("a switcher whose menu is still downloading", () => {
  it("lets other keys through and opens the menu asked for once it arrives, after a failed first download", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/limits", page([]));
    const { user } = await renderApp(`/orgs/${ORG}/projects/${PROJECT}/limits`);
    const switcher = await within(screen.getByRole("navigation", { name: "Console" })).findByRole("button", {
      name: "Checkout assistant",
      description: "Switch project",
    });
    act(() => switcher.focus());
    await user.keyboard("{Tab}");
    expect(switcher).not.toHaveFocus();
    await user.keyboard("{Shift>}{Tab}{/Shift}");
    expect(switcher).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    act(() => menus.release());
    const menu = await screen.findByRole("menu");
    expect(within(menu).getByRole("menuitemradio", { name: /Checkout assistant/ })).toHaveAttribute("aria-checked", "true");
    expect(within(menu).getByRole("menuitem", { name: "All projects" })).toHaveAttribute("href", `/orgs/${ORG}/projects`);
  });
});
