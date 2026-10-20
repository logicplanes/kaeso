import { afterEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { signedIn } from "~/test/fixtures";
import { me } from "~/test/fixtures/instance-account";
import { ssoStart } from "~/test/fixtures/public";
import { apiError, noContent, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = "/settings/account/security";
const notifications = async () => within(await screen.findByRole("region", { name: "Notifications" }));
const links = (linked: boolean) => ({
  data: [{ org_id: "org_contoso", org_name: "Contoso", provider_name: "Contoso ID", linked, linked_at: linked ? "2026-10-15T12:00:00Z" : null }],
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("single sign-on links", () => {
  it("links the account at the organization's identity provider", async () => {
    signedIn();
    on("GET", "me", me());
    on("GET", "me/sso-links", links(false));
    on("POST", "me/sso-links", ssoStart("https://idp.contoso.test/authorize"));
    const assign = vi.spyOn(window.location, "assign").mockImplementation(() => undefined);
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("Not linked; link it to sign in through Contoso ID.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Link" }));
    await waitFor(() => expect(assign).toHaveBeenCalledWith("https://idp.contoso.test/authorize"));
    expect(requests("POST", "me/sso-links").map((r) => r.body)).toEqual([{ org: "org_contoso", next: PATH }]);
  });

  it("names the organization's SSO when its provider has no name, and keeps the link when asked not to", async () => {
    signedIn();
    on("GET", "me", me());
    on("GET", "me/sso-links", {
      data: [
        { org_id: "org_contoso", org_name: "Contoso", provider_name: null, linked: true, linked_at: "2026-10-15T12:00:00Z" },
        { org_id: "org_fabrikam", org_name: "Fabrikam", provider_name: null, linked: false, linked_at: null },
      ],
    });
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("Linked; you can sign in through its SSO.")).toBeInTheDocument();
    expect(screen.getByText("Not linked; link it to sign in through its SSO.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Unlink" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Unlink from Contoso's single sign-on?" });
    await user.click(within(confirm).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(requests("DELETE", "me/sso-links/:org")).toHaveLength(0);
  });

  it("unlinks after asking", async () => {
    signedIn();
    on("GET", "me", me());
    on("GET", "me/sso-links", links(true));
    on("DELETE", "me/sso-links/:org", noContent);
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("Linked; you can sign in through Contoso ID.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Unlink" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Unlink from Contoso's single sign-on?" });
    await user.click(within(confirm).getByRole("button", { name: "Unlink" }));
    expect(await (await notifications()).findByText("Unlinked from single sign-on")).toBeInTheDocument();
    expect(requests("DELETE", "me/sso-links/:org").map((r) => r.params.org)).toEqual(["org_contoso"]);
  });

  it("says why an unlink was refused", async () => {
    signedIn();
    on("GET", "me", me());
    on("GET", "me/sso-links", links(true));
    on("DELETE", "me/sso-links/:org", apiError(404, "not_found", "No such link, or you cannot see it."));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Unlink" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Unlink from Contoso's single sign-on?" });
    await user.click(within(confirm).getByRole("button", { name: "Unlink" }));
    expect((await (await notifications()).findAllByText("No such link, or you cannot see it.")).length).toBeGreaterThan(0);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });

  it("says why linking failed when the provider sends the browser back", async () => {
    signedIn();
    on("GET", "me", me());
    on("GET", "me/sso-links", links(false));
    await renderApp(`${PATH}#sso_error=linked_elsewhere`);
    expect((await (await notifications()).findAllByText("This identity is already linked to another Kaeso account.")).length).toBeGreaterThan(0);
  });

  it("shows no section without an organization that uses single sign-on", async () => {
    signedIn();
    on("GET", "me", me());
    on("GET", "me/sso-links", { data: [] });
    await renderApp(PATH);
    expect(await screen.findByText("Changing it signs out your other sessions.")).toBeInTheDocument();
    await waitFor(() => expect(requests("GET", "me/sso-links")).toHaveLength(1));
    expect(screen.queryByText("Single sign-on")).not.toBeInTheDocument();
  });
});
