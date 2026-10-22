import { describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";
import { ORG, signedOut } from "~/test/fixtures";
import { methods, ssoStart } from "~/test/fixtures/public";
import { on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

describe("the first page a start link opens", () => {
  it("fetches the provider's step and goes on to the provider", async () => {
    signedOut();
    on(
      "GET",
      "auth/methods",
      methods({
        password: false,
        magic_link: false,
        sso: true,
        provider: { org: ORG, org_name: "Northwind Labs", name: "Northwind ID", auto_redirect: true },
      }),
    );
    on("POST", "auth/oidc/start", ssoStart("https://id.northwind.test/authorize?state=s8"));
    const assign = vi.spyOn(window.location, "assign").mockImplementation(() => undefined);
    await renderApp(`/sign-in?org=${ORG}&start=sso`);
    expect(await screen.findByRole("button", { name: "Continue with Northwind ID" })).toBeInTheDocument();
    await waitFor(() => expect(assign).toHaveBeenCalledWith("https://id.northwind.test/authorize?state=s8"));
    expect(requests("POST", "auth/oidc/start").map((r) => r.body)).toEqual([{ org: ORG }]);
    expect(screen.queryByRole("textbox", { name: "Email" })).not.toBeInTheDocument();
  });
});
