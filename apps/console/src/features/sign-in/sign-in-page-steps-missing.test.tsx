import { describe, expect, it, vi } from "vitest";
import { screen } from "@testing-library/react";
import { ORG, signedOut } from "~/test/fixtures";
import { methods } from "~/test/fixtures/public";
import { on } from "~/test/network";
import { renderApp } from "~/test/render";

vi.mock("./sign-in-steps", () => {
  throw new Error("The chunk couldn't be fetched.");
});

describe("signing in when the provider's step can't be fetched", () => {
  it("asks for the address instead", async () => {
    signedOut();
    on("GET", "auth/methods", methods({ password: false, magic_link: false, sso: true, provider: { org: ORG, name: "Northwind ID", auto_redirect: true } }));
    await renderApp("/sign-in");
    expect(await screen.findByRole("textbox", { name: "Email" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Continue with Northwind ID" })).not.toBeInTheDocument();
  });
});
