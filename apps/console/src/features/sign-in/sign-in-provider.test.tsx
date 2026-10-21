import { describe, expect, it } from "vitest";
import { screen } from "@testing-library/react";
import { ORG, signedOut } from "~/test/fixtures";
import { methods } from "~/test/fixtures/public";
import { on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

describe("the first sign-in page of an install that signs in only with single sign-on", () => {
  it("fetches the provider's step and offers it", async () => {
    signedOut();
    on(
      "GET",
      "auth/methods",
      methods({
        password: false,
        magic_link: false,
        sso: true,
        provider: { org: ORG, org_name: "Northwind Labs", name: "Northwind ID", auto_redirect: false },
      }),
    );
    await renderApp("/sign-in");
    expect(await screen.findByRole("button", { name: "Continue with Northwind ID" })).toHaveFocus();
    expect(screen.queryByRole("textbox", { name: "Email" })).not.toBeInTheDocument();
    expect(requests("POST", "auth/oidc/start")).toHaveLength(0);
  });
});
