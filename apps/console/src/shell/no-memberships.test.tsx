import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { membership, myMembership, signedIn, whoAmI } from "~/test/fixtures";
import { notifications } from "~/test/fixtures/org";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

function noOrgs(canCreate = true) {
  signedIn({ session: whoAmI({ memberships: [], can_create_orgs: canCreate }), memberships: [] });
}

describe("the welcome page for people outside every organization", () => {
  it("offers to create an organization and opens it", async () => {
    noOrgs();
    let created = false;
    const initech = { org_id: "org_new", org_name: "Initech" };
    on("GET", "auth/session", () => whoAmI({ memberships: created ? [membership(initech)] : [] }));
    on("GET", "me/memberships", () => ({ data: created ? [myMembership({ ...initech, projects: [] })] : [] }));
    on("POST", "me/orgs", () => {
      created = true;
      return { id: "org_new", name: "Initech" };
    });
    const { user, router } = await renderApp("/");
    expect(await screen.findByRole("heading", { name: "Welcome to Kaeso", level: 1 })).toBeInTheDocument();
    expect(document.title).toBe("Welcome · Kaeso");
    expect(screen.getByText("Create one to start, or ask an owner or admin to invite you to theirs.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Ada Okafor: account and sign out" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Create organization" }));
    const dialog = await screen.findByRole("dialog", { name: "Create organization" });
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(requests("POST", "me/orgs")).toHaveLength(0);
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Initech");
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(await notifications().findByText("Organization created")).toBeInTheDocument();
    await waitFor(() => expect(router.state.location.pathname).toBe("/orgs/org_new"));
    expect(requests("POST", "me/orgs").map((r) => r.body)).toEqual([{ name: "Initech" }]);
  });

  it("keeps the dialog open when the name is refused", async () => {
    noOrgs();
    on("POST", "me/orgs", apiError(409, "conflict", "An organization with this name exists."));
    const { user } = await renderApp("/");
    await user.click(await screen.findByRole("button", { name: "Create organization" }));
    const dialog = await screen.findByRole("dialog", { name: "Create organization" });
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Initech");
    await user.click(within(dialog).getByRole("button", { name: "Create organization" }));
    expect(await notifications().findAllByText("An organization with this name exists.")).not.toHaveLength(0);
    expect(screen.getByRole("dialog", { name: "Create organization" })).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Create organization" })).not.toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Create organization" }));
    await screen.findByRole("dialog", { name: "Create organization" });
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Create organization" })).not.toBeInTheDocument());
    expect(requests("POST", "me/orgs")).toHaveLength(1);
  });

  it("asks to be invited when the person can't create organizations", async () => {
    noOrgs(false);
    await renderApp("/");
    expect(await screen.findByText("Ask an owner or admin of an organization to invite you.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Create organization" })).not.toBeInTheDocument();
  });
});
