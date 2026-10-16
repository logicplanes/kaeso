import { screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { signedIn } from "~/test/fixtures";
import { me } from "~/test/fixtures/instance-account";
import { on } from "~/test/network";
import { renderApp } from "~/test/render";

const PROFILE = "/settings/account/profile";

vi.hoisted(() => window.history.replaceState(null, "", "/settings/account/profile"));

describe("opening the console on a settings page", () => {
  it("shows the settings sidebar", async () => {
    signedIn();
    on("GET", "me", me());
    await renderApp(PROFILE);
    const nav = await screen.findByRole("navigation", { name: "Settings" });
    expect(await within(nav).findByRole("link", { name: "Profile" })).toHaveAttribute("aria-current", "page");
  });
});
