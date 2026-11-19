import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import type { LinkedBillingApp, LinkedBillingView } from "~/api/generated/types";
import { signedIn, user as account, whoAmI } from "~/test/fixtures";
import { apiError, noContent, on, requests } from "~/test/network";
import { hoverReason } from "~/test/fixtures/org";
import { renderApp } from "~/test/render";

const PATH = "/settings/instance/linked-billing";

function offer(app: LinkedBillingApp | null = null): LinkedBillingView {
  return {
    provider: "anthropic",
    provider_name: "Anthropic",
    organization: "Claude organization",
    apps_url: "https://platform.claude.com/settings/oauth-apps",
    docs_url: "https://platform.claude.com/docs/en/manage-claude/oauth-apps/organization-billing-connections",
    redirect_uri: "https://kaeso.example.com/linked-billing/callback",
    issuer: "https://kaeso.example.com/platform/v1/linked-billing/oidc",
    jwks_url: "https://kaeso.example.com/platform/v1/linked-billing/oidc/jwks.json",
    jwks: { keys: [{ kty: "EC", crv: "P-256", kid: "k1", x: "x", y: "y" }] } as unknown as LinkedBillingView["jwks"],
    subject: "kaeso:linked-billing:anthropic",
    app,
    linked_keys: app ? 2 : 0,
  };
}

const app = (overrides: Partial<LinkedBillingApp> = {}): LinkedBillingApp => ({
  client_id: "oaapp_01EXAMPLE",
  audience: "aud",
  organization_id: "00000000-0000-4000-8000-000000000001",
  federation_rule_id: "fdrl_01EXAMPLE",
  workspace_id: null,
  service_account_id: null,
  send_billing_policy: false,
  checked_at: null,
  check_problem: null,
  updated_at: "2026-11-17T12:00:00Z",
  ...overrides,
});

function admin() {
  signedIn({ session: whoAmI({ user: account({ is_instance_admin: true }) }) });
}

describe("linked billing on the instance", () => {
  it("shows what to register and saves the install's app", async () => {
    admin();
    let current = offer();
    on("GET", "instance/linked-billing", () => [current]);
    on("PUT", "instance/linked-billing/:provider", ({ body }: { body: Record<string, unknown> }) => {
      current = offer(app({ client_id: String(body.client_id), workspace_id: (body.workspace_id as string | undefined) ?? null }));
      return current;
    });
    const { user } = await renderApp(PATH);
    expect(await screen.findByRole("heading", { name: "Register the app at Anthropic" })).toBeInTheDocument();
    expect(screen.getByText("https://kaeso.example.com/linked-billing/callback")).toBeInTheDocument();
    expect(screen.getByText("kaeso:linked-billing:anthropic")).toBeInTheDocument();
    expect(screen.getByText("Until it is set, organizations can't link.")).toBeInTheDocument();
    const save = screen.getByRole("button", { name: "Save" });
    expect(save).toHaveAttribute("aria-disabled", "true");
    await user.type(screen.getByLabelText("Client ID"), "oaapp_01EXAMPLE");
    await user.type(screen.getByLabelText("Audience"), "aud");
    await user.type(screen.getByLabelText("Organization ID"), "00000000-0000-4000-8000-000000000001");
    await user.type(screen.getByLabelText("Federation rule"), "fdrl_01EXAMPLE");
    await user.type(screen.getByLabelText("Workspace ID"), "wrkspc_1");
    await user.type(screen.getByLabelText("Service account ID"), "svac_1");
    await user.click(screen.getByRole("switch", { name: "Name the linked profile on each request" }));
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(requests("PUT", "instance/linked-billing/:provider")[0]?.body).toEqual({
        client_id: "oaapp_01EXAMPLE",
        audience: "aud",
        organization_id: "00000000-0000-4000-8000-000000000001",
        federation_rule_id: "fdrl_01EXAMPLE",
        workspace_id: "wrkspc_1",
        service_account_id: "svac_1",
        send_billing_policy: true,
      }),
    );
    expect(await screen.findByText("2 linked keys use it.")).toBeInTheDocument();
  });

  it("saves only the fields given and shows a field the gateway refused", async () => {
    admin();
    on("GET", "instance/linked-billing", [offer()]);
    on(
      "PUT",
      "instance/linked-billing/:provider",
      apiError(400, "invalid_request", "`client_id` must be 1 to 200 characters without spaces.", { param: "client_id" }),
    );
    const { user } = await renderApp(PATH);
    await user.type(await screen.findByLabelText("Client ID"), "oaapp");
    await hoverReason(user, screen.getByRole("button", { name: "Save" }), "Fill in the client ID, audience, organization and rule");
    await user.type(screen.getByLabelText("Audience"), "aud");
    await user.type(screen.getByLabelText("Organization ID"), "org");
    await user.type(screen.getByLabelText("Federation rule"), "fdrl_1");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByText("`client_id` must be 1 to 200 characters without spaces.")).toBeInTheDocument();
    expect(requests("PUT", "instance/linked-billing/:provider")[0]?.body).toEqual({
      client_id: "oaapp",
      audience: "aud",
      organization_id: "org",
      federation_rule_id: "fdrl_1",
      send_billing_policy: false,
    });
  });

  it("checks the app and says what is wrong", async () => {
    admin();
    let current = offer(app());
    on("GET", "instance/linked-billing", () => [current]);
    on("POST", "instance/linked-billing/:provider/check", () => {
      current = offer(app({ checked_at: "2026-11-17T12:05:00Z", check_problem: "client" }));
      return current;
    });
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("Not checked yet")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Check" }));
    expect(await screen.findByText("Failed")).toBeInTheDocument();
    expect(screen.getAllByText("Anthropic refused the client; check the client ID, audience and issuer.").length).toBeGreaterThan(0);
  });

  it.each([
    ["federation", "Anthropic refused the service account token; check the issuer and the rule."],
    ["scope", "The app lacks billing:attribution or Offline access."],
    ["unauthorized", "The app has no operator service account or workspace."],
    ["unreachable", "Anthropic couldn't be reached; try again shortly."],
    ["request", "Anthropic refused the link request; check the redirect URI."],
  ])("names the problem %s a check found", async (problem, text) => {
    admin();
    on("GET", "instance/linked-billing", [offer(app({ checked_at: "2026-11-17T12:05:00Z", check_problem: problem }))]);
    await renderApp(PATH);
    expect(await screen.findByText(text)).toBeInTheDocument();
  });

  it("replaces and removes the app", async () => {
    admin();
    on("GET", "instance/linked-billing", () => [
      offer(app({ checked_at: "2026-11-17T12:05:00Z", workspace_id: "wrkspc_1", service_account_id: "svac_1", send_billing_policy: true })),
    ]);
    on("DELETE", "instance/linked-billing/:provider", noContent());
    on("POST", "instance/linked-billing/:provider/check", offer(app({ checked_at: "2026-11-17T12:06:00Z" })));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Check" }));
    await waitFor(() => expect(requests("POST", "instance/linked-billing/:provider/check")).toHaveLength(1));
    await user.click(screen.getByRole("button", { name: "Replace" }));
    expect(screen.getByLabelText("Client ID")).toHaveValue("oaapp_01EXAMPLE");
    expect(screen.getByLabelText("Workspace ID")).toHaveValue("wrkspc_1");
    expect(screen.getByLabelText("Service account ID")).toHaveValue("svac_1");
    expect(screen.getByRole("switch", { name: "Name the linked profile on each request" })).toBeChecked();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    await user.click(screen.getByRole("button", { name: "Remove" }));
    await waitFor(() => expect(requests("DELETE", "instance/linked-billing/:provider")).toHaveLength(1));
  });

  it("says when linked billing can't load", async () => {
    admin();
    on("GET", "instance/linked-billing", () => new Response(null, { status: 500 }));
    await renderApp(PATH);
    expect(await screen.findByText("Linked billing couldn't load")).toBeInTheDocument();
    expect(within(document.body).queryByText("Register the app at Anthropic")).toBeNull();
  });
});
