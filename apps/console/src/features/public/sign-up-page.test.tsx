import { describe, expect, it } from "vitest";
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { ORG, signedIn } from "~/test/fixtures";
import { emailSent, methods, signedInReply } from "~/test/fixtures/public";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const notifications = async () => within(await screen.findByRole("region", { name: "Notifications" }));

describe("creating an account", () => {
  it("says accounts are by invitation when the install doesn't allow sign-up", async () => {
    on("GET", "auth/methods", methods({ sign_up: false }));
    await renderApp("/sign-up");
    expect(await screen.findByText("Accounts here are by invitation")).toBeInTheDocument();
    expect(screen.getByText("Ask an administrator to invite you.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Back to sign in" })).toHaveAttribute("href", "/sign-in");
  });

  it("emails a link to finish the account", async () => {
    on("GET", "auth/methods", methods({ sign_up: true }));
    on("POST", "auth/signup", emailSent("We sent a link to grace@example.com to finish your account."));
    const { user } = await renderApp("/sign-up");
    expect(await screen.findByText("We email you a link to finish your account.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Sign in" })).toHaveAttribute("href", "/sign-in");
    const go = screen.getByRole("button", { name: "Continue" });
    expect(go).toBeDisabled();
    await user.type(screen.getByRole("textbox", { name: "Email" }), "  grace@example.com ");
    await user.click(go);
    expect(await screen.findByText("Check your email")).toBeInTheDocument();
    expect(screen.getByText("We sent a link to grace@example.com to finish your account.")).toBeInTheDocument();
    expect(requests("POST", "auth/signup").map((r) => r.body)).toEqual([{ email: "grace@example.com" }]);
    expect(screen.getByRole("link", { name: "Back to sign in" })).toBeInTheDocument();
  });

  it("shows the refusal and keeps the address", async () => {
    on("GET", "auth/methods", methods({ sign_up: true }));
    on("POST", "auth/signup", apiError(422, "invalid_request", "Addresses at elsewhere.test can't sign up here.", { param: "email" }));
    const { user } = await renderApp("/sign-up");
    await user.type(await screen.findByRole("textbox", { name: "Email" }), "grace@elsewhere.test{Enter}");
    expect((await (await notifications()).findAllByText("Addresses at elsewhere.test can't sign up here.")).length).toBeGreaterThan(0);
    expect(screen.getByRole("textbox", { name: "Email" })).toHaveAccessibleDescription("Addresses at elsewhere.test can't sign up here.");
    expect(screen.getByRole("textbox", { name: "Email" })).toHaveValue("grace@elsewhere.test");
  });

  it("asks to try again when the gateway's answer can't be read", async () => {
    on("GET", "auth/methods", methods({ sign_up: true }));
    on("POST", "auth/signup", () => new Response("{", { status: 200, headers: { "content-type": "application/json" } }));
    const { user } = await renderApp("/sign-up");
    await user.type(await screen.findByRole("textbox", { name: "Email" }), "grace@example.com{Enter}");
    expect((await (await notifications()).findAllByText("Can't reach the gateway; try again.")).length).toBeGreaterThan(0);
    expect(screen.getByRole("textbox", { name: "Email" })).toHaveValue("grace@example.com");
  });
});

describe("finishing an account", () => {
  it("asks for the sign-up link when it's missing", async () => {
    await renderApp("/auth/signup");
    expect(await screen.findByText("This page needs the sign-up link")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Start again" })).toHaveAttribute("href", "/sign-up");
  });

  it("creates the account with a name and a password", async () => {
    on("POST", "auth/signup/verify", () => {
      signedIn();
      return signedInReply();
    });
    const { user } = await renderApp("/auth/signup#token=sgn_1");
    expect(await screen.findByText("Choose the name people see; a password is optional.")).toBeInTheDocument();
    expect(window.location.hash).toBe("");
    expect(screen.getByLabelText(/^Password/)).toHaveAccessibleDescription("Leave empty to sign in with a link by email.");
    const create = screen.getByRole("button", { name: "Create account" });
    expect(create).toBeDisabled();
    await user.type(screen.getByRole("textbox", { name: "Name" }), " Grace Hopper ");
    await user.type(screen.getByLabelText(/Password/), "short");
    expect(screen.getByText("7 more characters to go.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Create account" })).toBeDisabled();
    await user.type(screen.getByLabelText(/Password/), " and longer{Enter}");
    await waitFor(() => expect(window.location.pathname).toMatch(new RegExp(`^/orgs/${ORG}`)));
    expect(requests("POST", "auth/signup/verify").map((r) => r.body)).toEqual([{ token: "sgn_1", name: "Grace Hopper", password: "short and longer" }]);
  });

  it("creates the account without a password", async () => {
    on("POST", "auth/signup/verify", () => {
      signedIn();
      return signedInReply();
    });
    const { user } = await renderApp("/auth/signup#token=sgn_1");
    await user.type(await screen.findByRole("textbox", { name: "Name" }), "Grace Hopper");
    await user.click(screen.getByRole("button", { name: "Create account" }));
    await waitFor(() => expect(requests("POST", "auth/signup/verify").map((r) => r.body)).toEqual([{ token: "sgn_1", name: "Grace Hopper" }]));
    await waitFor(() => expect(window.location.pathname).toMatch(new RegExp(`^/orgs/${ORG}`)));
  });

  it("ignores Enter until the form is ready", async () => {
    const { user } = await renderApp("/auth/signup#token=sgn_1");
    const name = await screen.findByRole("textbox", { name: "Name" });
    await user.type(name, "   {Enter}");
    fireEvent.submit(name.closest("form") as HTMLFormElement);
    expect(requests("POST", "auth/signup/verify")).toHaveLength(0);
  });

  it("shows why the link was refused and offers to start again", async () => {
    on("POST", "auth/signup/verify", apiError(401, "authentication", "This sign-up link has expired."));
    const { user } = await renderApp("/auth/signup#token=sgn_1");
    await user.type(await screen.findByRole("textbox", { name: "Name" }), "Grace Hopper{Enter}");
    expect((await (await notifications()).findAllByText("This sign-up link has expired.")).length).toBeGreaterThan(0);
    expect(await screen.findByText("This link has expired or was already used.")).toBeInTheDocument();
    expect(screen.getByText("A sign-up link works once; start again for a new one.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Start again" })).toHaveAttribute("href", "/sign-up");
    expect(window.location.pathname).toBe("/auth/signup");
  });

  it("marks the field the gateway refused", async () => {
    on("POST", "auth/signup/verify", apiError(422, "invalid_request", "Use at most 100 characters.", { param: "name" }));
    const { user } = await renderApp("/auth/signup#token=sgn_1");
    await user.type(await screen.findByRole("textbox", { name: "Name" }), "Grace Hopper{Enter}");
    expect((await (await notifications()).findAllByText("Use at most 100 characters.")).length).toBeGreaterThan(0);
    expect(screen.getByRole("textbox", { name: "Name" })).toHaveAccessibleDescription("Use at most 100 characters.");
    expect(screen.getByLabelText(/Password/)).not.toHaveAttribute("aria-invalid");
  });
});
