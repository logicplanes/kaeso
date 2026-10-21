import { describe, expect, it } from "vitest";
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { signedOut } from "~/test/fixtures";
import { emailSent, methods } from "~/test/fixtures/public";
import { apiError, noContent, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const notifications = async () => within(await screen.findByRole("region", { name: "Notifications" }));

describe("requesting a password reset", () => {
  it("emails a reset link to the trimmed address", async () => {
    on("POST", "auth/password-reset", emailSent("Sent."));
    const { user } = await renderApp("/auth/password-reset");
    expect(await screen.findByText("We email you a link to set a new one, valid for 15 minutes.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Back to sign in" })).toHaveAttribute("href", "/sign-in");
    const send = screen.getByRole("button", { name: "Email me a link" });
    expect(send).toBeDisabled();
    await user.type(screen.getByRole("textbox", { name: "Email" }), " ada@example.com ");
    await user.click(send);
    expect(await screen.findByText("Check your email")).toBeInTheDocument();
    expect(screen.getByText("If an account exists for ada@example.com, a reset link is on its way.")).toBeInTheDocument();
    expect(requests("POST", "auth/password-reset").map((r) => r.body)).toEqual([{ email: "ada@example.com" }]);
  });

  it("starts from the address typed on the sign-in page", async () => {
    signedOut();
    on("GET", "auth/methods", methods());
    on("POST", "auth/methods", methods());
    const { user } = await renderApp("/sign-in");
    await user.type(await screen.findByRole("textbox", { name: "Email" }), "ada@example.com{Enter}");
    await user.click(await screen.findByRole("link", { name: "Forgot your password?" }));
    expect(await screen.findByRole("textbox", { name: "Email" })).toHaveValue("ada@example.com");
    expect(screen.getByRole("button", { name: "Email me a link" })).toBeEnabled();
  });

  it("sets the new password when the link from the email opens in the same tab", async () => {
    on("POST", "auth/password-reset", emailSent("Sent."));
    const { user } = await renderApp("/auth/password-reset");
    await user.type(await screen.findByRole("textbox", { name: "Email" }), "ada@example.com{Enter}");
    expect(await screen.findByText("Check your email")).toBeInTheDocument();
    window.history.pushState(null, "", "/auth/password-reset#token=rst_2");
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    expect(await screen.findByRole("heading", { name: "Set a new password", level: 1 })).toBeInTheDocument();
    expect(screen.queryByText("Check your email")).not.toBeInTheDocument();
    expect(window.location.hash).toBe("");
  });

  it("shows the refusal", async () => {
    on("POST", "auth/password-reset", apiError(429, "rate_limited", "Too many reset requests; try again in a few minutes."));
    const { user } = await renderApp("/auth/password-reset");
    await user.type(await screen.findByRole("textbox", { name: "Email" }), "ada@example.com{Enter}");
    expect((await (await notifications()).findAllByText("Too many reset requests; try again in a few minutes.")).length).toBeGreaterThan(0);
    expect(screen.getByRole("textbox", { name: "Email" })).toHaveValue("ada@example.com");
  });
});

describe("setting a new password", () => {
  it("sets the password and sends the person to sign in with a notice", async () => {
    signedOut();
    on("GET", "auth/methods", methods());
    on("POST", "auth/password-reset/verify", noContent);
    const { user } = await renderApp("/auth/password-reset#token=rst_1");
    expect(await screen.findByText("Every session of your account ends; sign in again with the new one.")).toBeInTheDocument();
    expect(window.location.hash).toBe("");
    const field = screen.getByLabelText("New password");
    await user.type(field, "too short{Enter}");
    fireEvent.submit(field.closest("form") as HTMLFormElement);
    expect(requests("POST", "auth/password-reset/verify")).toHaveLength(0);
    expect(screen.getByRole("button", { name: "Set password" })).toBeDisabled();
    await user.type(field, " but now long");
    await user.click(screen.getByRole("button", { name: "Set password" }));
    expect(await (await notifications()).findByText("Password changed; sign in with the new one")).toBeInTheDocument();
    expect(window.location.pathname).toBe("/sign-in");
    expect(requests("POST", "auth/password-reset/verify").map((r) => r.body)).toEqual([{ token: "rst_1", password: "too short but now long" }]);
  });

  it("names the account for password managers and keeps the hint's height as the password grows", async () => {
    on("POST", "auth/password-reset/lookup", { email: "ada@example.com", expires_at: "2026-10-20T12:15:00Z" });
    const { user } = await renderApp("/auth/password-reset#token=rst_1");
    const field = await screen.findByLabelText("New password");
    await waitFor(() => expect(document.querySelector('input[autocomplete="username"]')).toHaveValue("ada@example.com"));
    expect(requests("POST", "auth/password-reset/lookup").map((r) => r.body)).toEqual([{ token: "rst_1" }]);
    expect(field).toHaveAttribute("autocomplete", "new-password");
    await user.type(field, "short");
    expect(field).toHaveAccessibleDescription("7 more characters to go.");
    const rule = screen.getByText("12 to 128 characters. A few unrelated words are easy to remember and hard to guess.");
    expect(rule).toHaveAttribute("aria-hidden");
    expect(rule.parentElement).toContainElement(screen.getByText("7 more characters to go."));
  });

  it("says a used link has expired before anyone types a password", async () => {
    on("POST", "auth/password-reset/lookup", apiError(401, "authentication", "This reset link is invalid, used or expired; request a new one."));
    await renderApp("/auth/password-reset#token=rst_1");
    expect(await screen.findByText("This link has expired or was already used.")).toBeInTheDocument();
    expect(screen.queryByLabelText("New password")).not.toBeInTheDocument();
  });

  it("offers a new link when the reset link has expired", async () => {
    on("POST", "auth/password-reset/verify", apiError(401, "authentication", "This reset link has expired."));
    const { user } = await renderApp("/auth/password-reset#token=rst_1");
    await user.type(await screen.findByLabelText("New password"), "correct horse battery{Enter}");
    expect(await screen.findByText("This link has expired or was already used.")).toBeInTheDocument();
    expect(screen.getByText("A reset link works once, for 15 minutes.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Request a new link" })).toHaveAttribute("href", "/auth/password-reset");
  });

  it("keeps the form when the password is refused", async () => {
    on("POST", "auth/password-reset/verify", apiError(422, "invalid_request", "That password appears in known breaches.", { param: "password" }));
    const { user } = await renderApp("/auth/password-reset#token=rst_1");
    await user.type(await screen.findByLabelText("New password"), "correct horse battery{Enter}");
    expect((await (await notifications()).findAllByText("That password appears in known breaches.")).length).toBeGreaterThan(0);
    await waitFor(() => expect(screen.getByRole("button", { name: "Set password" })).toBeEnabled());
    expect(screen.getByLabelText("New password")).toHaveAttribute("aria-invalid", "true");
    expect(screen.getByLabelText("New password")).toHaveAccessibleDescription(/That password appears in known breaches\./);
  });
});
