import { describe, expect, it, vi } from "vitest";
import { screen } from "@testing-library/react";
import { signedOut } from "~/test/fixtures";
import { methods } from "~/test/fixtures/public";
import { on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

vi.mock("./sign-up-line", () => {
  throw new Error("The chunk couldn't be fetched.");
});

describe("signing in when the sign-up line can't be fetched", () => {
  it("still signs in, without offering to create an account", async () => {
    signedOut();
    on("GET", "auth/methods", methods({ sign_up: true }));
    on("POST", "auth/methods", methods());
    const { user } = await renderApp("/sign-in");
    await user.type(await screen.findByRole("textbox", { name: "Email" }), "ada@example.com");
    expect(screen.queryByRole("link", { name: "Create an account" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Continue" }));
    expect(await screen.findByLabelText("Password")).toBeInTheDocument();
    expect(requests("POST", "auth/methods").map((r) => r.body)).toEqual([{ email: "ada@example.com" }]);
  });
});
