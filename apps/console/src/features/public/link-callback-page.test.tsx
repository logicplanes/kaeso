import { describe, expect, it } from "vitest";
import { screen, waitFor } from "@testing-library/react";
import { ORG, signedIn } from "~/test/fixtures";
import { credential } from "~/test/fixtures/org";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { rememberLink } from "~/kaeso/linked-billing";

const BACK = "/linked-billing/callback?code=code-1&state=st-1";

describe("the linked billing callback", () => {
  it("finishes the link it started and opens Providers", async () => {
    signedIn();
    on("GET", "orgs/:org/credentials", { data: [] });
    on(
      "POST",
      "orgs/:org/credentials/oauth/:attempt",
      credential({ id: "cred_link", name: "Acme Claude", provider: "anthropic", auth_method: "linked_billing" }),
    );
    rememberLink({ org: ORG, attempt: "att_1" });
    await renderApp(BACK);
    await waitFor(() => expect(window.location.pathname).toBe(`/orgs/${ORG}/providers`));
    const [sent] = requests("POST", "orgs/:org/credentials/oauth/:attempt");
    expect(sent?.params.attempt).toBe("att_1");
    expect((sent?.body as { callback_url: string }).callback_url).toContain("code=code-1&state=st-1");
    expect(window.location.search).toBe("");
  });

  it("finishes linking an existing key again", async () => {
    signedIn();
    on("GET", "orgs/:org/credentials", { data: [] });
    on("POST", "orgs/:org/credentials/:credential/oauth/:attempt", credential({ id: "cred_link", provider: "anthropic", auth_method: "linked_billing" }));
    rememberLink({ org: ORG, attempt: "att_2", credential: "cred_link" });
    await renderApp(BACK);
    await waitFor(() => expect(requests("POST", "orgs/:org/credentials/:credential/oauth/:attempt")).toHaveLength(1));
    expect(requests("POST", "orgs/:org/credentials/:credential/oauth/:attempt")[0]?.params.credential).toBe("cred_link");
  });

  it("says why a link didn't finish", async () => {
    signedIn();
    on(
      "POST",
      "orgs/:org/credentials/oauth/:attempt",
      apiError(400, "invalid_request", "Linking didn't complete; sign in with an account that has the Admin or Billing role.", { param: "callback_url" }),
    );
    rememberLink({ org: ORG, attempt: "att_3" });
    await renderApp(BACK);
    expect(await screen.findByText("Linking didn't finish")).toBeInTheDocument();
    expect(screen.getByText("Linking didn't complete; sign in with an account that has the Admin or Billing role.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Back to Providers" })).toHaveAttribute("href", `/orgs/${ORG}/providers`);
  });

  it("says when the gateway can't be reached", async () => {
    signedIn();
    on("POST", "orgs/:org/credentials/oauth/:attempt", () => {
      throw new TypeError("offline");
    });
    rememberLink({ org: ORG, attempt: "att_4" });
    await renderApp(BACK);
    expect(await screen.findByText("Linking didn't finish")).toBeInTheDocument();
  });

  it("asks to start again when no link is waiting", async () => {
    signedIn();
    await renderApp(BACK);
    expect(await screen.findByText("This link can't be finished here")).toBeInTheDocument();
    expect(requests("POST")).toHaveLength(0);
  });
});
