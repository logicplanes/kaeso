import { describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";
import { ORG, signedOut } from "~/test/fixtures";
import { methods, ssoStart } from "~/test/fixtures/public";
import { on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

describe("the first page a person signs in on", () => {
  it("goes on to a provider that starts without a click only after the address", async () => {
    signedOut();
    on("GET", "auth/methods", methods({ password: false, magic_link: false }));
    on(
      "POST",
      "auth/methods",
      methods({
        password: false,
        magic_link: false,
        provider: { org: ORG, org_name: "Northwind Labs", name: "Northwind ID", auto_redirect: true },
      }),
    );
    on("POST", "auth/oidc/start", ssoStart("https://id.northwind.test/authorize?state=s8"));
    const assign = vi.spyOn(window.location, "assign").mockImplementation(() => undefined);
    const { user } = await renderApp("/sign-in");
    await user.type(await screen.findByRole("textbox", { name: "Email" }), "ada@northwind.test{Enter}");
    await waitFor(() => expect(assign).toHaveBeenCalledWith("https://id.northwind.test/authorize?state=s8"));
    expect(requests("POST", "auth/oidc/start").map((r) => r.body)).toEqual([{ email: "ada@northwind.test" }]);
    expect(screen.queryByRole("button", { name: "Continue with Northwind ID" })).not.toBeInTheDocument();
  });
});
