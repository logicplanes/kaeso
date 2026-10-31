import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ORG, PROJECT, membership, signedIn, user as person, whoAmI } from "~/test/fixtures";
import { me } from "~/test/fixtures/instance-account";
import { member } from "~/test/fixtures/org";
import { on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PROFILE = "/settings/account/profile";

async function sidebar() {
  return screen.findByRole("navigation", { name: "Settings" });
}

const links = (nav: HTMLElement) =>
  within(nav)
    .getAllByRole("link")
    .map((l) => l.getAttribute("aria-label") ?? l.textContent);

describe("the settings sidebar", () => {
  it("lists the account, organization and instance pages and leads back to the console", async () => {
    signedIn();
    on("GET", "me", me());
    window.localStorage.setItem("kaeso:last", JSON.stringify({ org: ORG, project: PROJECT }));
    await renderApp(PROFILE);
    const nav = await sidebar();
    await within(nav).findByRole("link", { name: "Profile" });
    expect(within(nav).getByRole("link", { name: "Profile" })).toHaveAttribute("aria-current", "page");
    expect(within(nav).getByRole("link", { name: "Back to app" })).toHaveAttribute("href", `/orgs/${ORG}/projects/${PROJECT}`);
    expect(within(nav).getByRole("link", { name: "Back to the console" })).toHaveAttribute("href", `/orgs/${ORG}/projects/${PROJECT}`);
    expect(within(nav).getByText("Northwind Labs")).toBeInTheDocument();
    expect(within(nav).getByRole("link", { name: "Platform keys" })).toHaveAttribute("href", `/settings/orgs/${ORG}/platform-keys`);
    expect(within(nav).getByRole("link", { name: "System" })).toHaveAttribute("href", "/settings/instance/system");
    expect(within(nav).queryByRole("link", { name: "Model sources" })).not.toBeInTheDocument();
    expect(within(nav).getAllByRole("link", { name: "Audit log" })).toHaveLength(2);
  });

  it("shows only the organization pages the person may see and no instance pages to members", async () => {
    signedIn({ session: whoAmI({ user: person({ is_instance_admin: false }), memberships: [membership({ rights: ["org.view"] })] }) });
    on("GET", "me", me());
    await renderApp(PROFILE);
    const nav = await sidebar();
    await within(nav).findByRole("link", { name: "General" });
    expect(links(nav)).toEqual([
      "Back to the console",
      "Back to app",
      "Profile",
      "Security",
      "Sessions",
      "API keys",
      "Agents",
      "Memberships",
      "Notifications",
      "Appearance",
      "General",
      "Security",
      "Logging",
    ]);
    expect(within(nav).getByRole("link", { name: "Back to app" })).toHaveAttribute("href", `/orgs/${ORG}`);
  });

  it("filters the pages by name and opens the first match with Enter", async () => {
    signedIn();
    on("GET", "me", me());
    const { user, router } = await renderApp(PROFILE);
    const nav = await sidebar();
    const search = within(nav).getByRole("searchbox", { name: "Search settings" });
    await user.type(search, "zzz");
    expect(within(nav).getByText("No settings match")).toBeInTheDocument();
    await user.clear(search);
    await user.type(search, "  sess  ");
    expect(links(nav)).toEqual(["Back to the console", "Back to app", "Sessions"]);
    await user.type(search, "{Enter}");
    await waitFor(() => expect(router.state.location.pathname).toBe("/settings/account/sessions"));
  });

  it("prefetches a page's data when the pointer rests on its link", async () => {
    signedIn();
    on("GET", "me", me());
    on("GET", "orgs/:org/members", page([member()]));
    const { user } = await renderApp(PROFILE);
    const nav = await sidebar();
    await user.hover(within(nav).getByRole("link", { name: "Members" }));
    await waitFor(() => expect(requests("GET", "orgs/:org/members")).toHaveLength(1));
  });
});
