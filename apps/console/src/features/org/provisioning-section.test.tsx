import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import type { ScimStatus } from "~/api/generated/types";
import { ORG, signedIn, withRights } from "~/test/fixtures";
import { hoverReason, notifications, security, sso } from "~/test/fixtures/org";
import { on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = `/settings/orgs/${ORG}/security`;
const BASE = `https://kaeso.example.com/platform/v1/orgs/${ORG}/scim/v2`;

const off: ScimStatus = { enabled: false, base_url: BASE, provisioned: 0 };
const on_: ScimStatus = {
  enabled: true,
  base_url: BASE,
  last4: "WxYz",
  created_at: "2026-11-01T00:00:00Z",
  last_used_at: "2026-11-01T00:00:00Z",
  last_error: "`active` must be true or false.",
  last_error_at: "2026-11-01T00:00:00Z",
  provisioned: 12,
};

describe("provisioning", () => {
  it("creates a token, shows it once, and deletes it", async () => {
    signedIn();
    on("GET", "orgs/:org/security", security({ sso: sso() }));
    let current = off;
    on("GET", "orgs/:org/scim", () => current);
    on("POST", "orgs/:org/scim/token", () => {
      current = on_;
      return { token: "sk-kso-scim-secret", scim: on_ };
    });
    on("DELETE", "orgs/:org/scim/token", () => {
      current = off;
      return null;
    });
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("No token: the identity provider can't provision.")).toBeInTheDocument();
    expect(screen.getAllByText(BASE).length).toBeGreaterThan(0);
    await user.click(screen.getByRole("button", { name: "Create token" }));
    const dialog = await screen.findByRole("dialog", { name: "SCIM token created" });
    expect(within(dialog).getByText("sk-kso-scim-secret")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Finish" }));
    expect(await screen.findByText("Ends in WxYz")).toBeInTheDocument();
    expect(screen.getByText(/12 members provisioned/)).toBeInTheDocument();
    expect(screen.getByText("`active` must be true or false.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Replace token" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Delete" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Delete the SCIM token?" });
    await user.click(within(confirm).getByRole("button", { name: "Delete" }));
    expect(await notifications().findByText("SCIM token deleted")).toBeInTheDocument();
    expect(requests("DELETE", "orgs/:org/scim/token")).toHaveLength(1);
    expect(await screen.findByRole("button", { name: "Create token" })).toBeInTheDocument();
  });

  it("shows a token the provider hasn't used yet, and closes the token with Escape", async () => {
    signedIn();
    on("GET", "orgs/:org/security", security({ sso: sso() }));
    on("GET", "orgs/:org/scim", { enabled: true, base_url: BASE, provisioned: 0, last_error: "Refused." });
    on("POST", "orgs/:org/scim/token", { token: "sk-kso-scim-again", scim: on_ });
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("No request yet")).toBeInTheDocument();
    expect(screen.getByText("Ends in")).toBeInTheDocument();
    expect(screen.getByText("Refused.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Replace token" }));
    await screen.findByRole("dialog", { name: "SCIM token created" });
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "SCIM token created" })).not.toBeInTheDocument());
  });

  it("needs single sign-on", async () => {
    signedIn();
    on("GET", "orgs/:org/security", security({ sso: null }));
    on("GET", "orgs/:org/scim", off);
    const { user } = await renderApp(PATH);
    await hoverReason(user, await screen.findByRole("button", { name: "Create token" }), "Connect an identity provider first");
  });

  it("needs the right to manage single sign-on", async () => {
    withRights(["org.view"], { role: "admin" });
    on("GET", "orgs/:org/security", security({ sso: sso() }));
    on("GET", "orgs/:org/scim", on_);
    const second = await renderApp(PATH);
    await screen.findByText("Ends in WxYz");
    await hoverReason(second.user, screen.getByRole("button", { name: "Replace token" }), /Manage SSO/);
    expect(screen.getByRole("button", { name: "Delete" })).toHaveAttribute("aria-disabled", "true");
  });
});
