import { afterEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { notifications } from "~/test/fixtures/org";
import { renderApp } from "~/test/render";
import { ORG, signedIn, whoAmI } from "~/test/fixtures";
import { me } from "~/test/fixtures/instance-account";
import { apiError, noContent, on, requests } from "~/test/network";

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("profile page", () => {
  it("opens from the settings root and shows the account", async () => {
    signedIn();
    on("GET", "me", me());
    await renderApp("/settings");
    expect(await screen.findByRole("textbox", { name: "Name" })).toHaveValue("Ada Okafor");
    expect(window.location.pathname).toBe("/settings/account/profile");
    expect(screen.getByText("owner@example.com")).toBeInTheDocument();
    expect(screen.getByText("usr_ada")).toBeInTheDocument();
    expect(screen.getByText("15 Oct 2026")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Upload" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Remove" })).not.toBeInTheDocument();
  });

  it("names the organization that manages the account", async () => {
    signedIn({ session: whoAmI({ managed_by: { org: ORG, name: "Acme" } }) });
    on("GET", "me", me());
    await renderApp("/settings/account/profile");
    expect(await screen.findByText("Managed by")).toBeInTheDocument();
    expect(screen.getByText("Acme")).toBeInTheDocument();
    expect(screen.getByText("Your address is at its domain; its SSO signs you in fully.")).toBeInTheDocument();
  });

  it("names no organization for an account none manages", async () => {
    signedIn();
    on("GET", "me", me());
    await renderApp("/settings/account/profile");
    await screen.findByRole("textbox", { name: "Name" });
    expect(screen.queryByText("Managed by")).not.toBeInTheDocument();
  });

  it("saves a new name from the button and from Enter", async () => {
    signedIn();
    on("GET", "me", me());
    on("PATCH", "me", ({ body }: { body: unknown }) => me(body as { name: string }));
    const { user } = await renderApp("/settings/account/profile");
    const name = await screen.findByRole("textbox", { name: "Name" });
    const save = screen.getByRole("button", { name: "Save" });
    expect(save).toBeDisabled();
    await user.clear(name);
    await user.type(name, "   ");
    expect(save).toBeDisabled();
    await user.keyboard("{Enter}");
    expect(requests("PATCH", "me")).toHaveLength(0);
    await user.clear(name);
    await user.type(name, "  Ada Lovelace  ");
    await user.click(save);
    expect(await notifications().findByText("Name saved")).toBeInTheDocument();
    await user.type(name, "{Enter}");
    await waitFor(() => expect(requests("PATCH", "me")).toHaveLength(2));
    expect(requests("PATCH", "me").map((r) => r.body)).toEqual([{ name: "Ada Lovelace" }, { name: "Ada Lovelace" }]);
  });

  it("sends the name once while the change is on its way", async () => {
    signedIn();
    on("GET", "me", me());
    let answer: (view: unknown) => void = () => {};
    on("PATCH", "me", () => new Promise((resolve) => (answer = resolve)));
    const { user } = await renderApp("/settings/account/profile");
    const name = await screen.findByRole("textbox", { name: "Name" });
    await user.clear(name);
    await user.type(name, "Ada Lovelace{Enter}{Enter}");
    answer(me({ name: "Ada Lovelace" }));
    expect(await notifications().findByText("Name saved")).toBeInTheDocument();
    expect(requests("PATCH", "me")).toHaveLength(1);
  });

  it("sends a confirmation link to a new email address", async () => {
    signedIn();
    on("GET", "me", me());
    on("POST", "me/email", { status: "sent" });
    const { user } = await renderApp("/settings/account/profile");
    await user.click(await screen.findByRole("button", { name: "Change email" }));
    const dialog = await screen.findByRole("dialog", { name: "Change email" });
    const send = within(dialog).getByRole("button", { name: "Send confirmation link" });
    expect(send).toBeDisabled();
    await user.type(within(dialog).getByLabelText("New email"), " ada@example.org ");
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(await notifications().findByText("Confirmation link sent; your old address was notified")).toBeInTheDocument();
    expect(requests("POST", "me/email")[0]?.body).toEqual({ email: "ada@example.org" });
    expect(screen.getByText("owner@example.com · link sent to ada@example.org")).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Change email" })).not.toBeInTheDocument());
  });

  it("stays open while the confirmation link is on its way, so it can't be sent twice", async () => {
    signedIn();
    on("GET", "me", me());
    let respond: (body: unknown) => void = () => undefined;
    on("POST", "me/email", () => new Promise((resolve) => (respond = resolve)));
    const { user } = await renderApp("/settings/account/profile");
    await user.click(await screen.findByRole("button", { name: "Change email" }));
    const dialog = await screen.findByRole("dialog", { name: "Change email" });
    await user.type(within(dialog).getByLabelText("New email"), "ada@example.org");
    await user.click(within(dialog).getByRole("button", { name: "Send confirmation link" }));
    await waitFor(() => expect(requests("POST", "me/email")).toHaveLength(1));
    expect(within(dialog).getByRole("button", { name: "Cancel" })).toBeDisabled();
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    within(dialog).getByLabelText("New email").focus();
    await user.keyboard("{Meta>}{Enter}{/Meta}{Escape}");
    expect(screen.getByRole("dialog", { name: "Change email" })).toBe(dialog);
    expect(requests("POST", "me/email")).toHaveLength(1);
    respond({ status: "sent" });
    expect(await notifications().findByText("Confirmation link sent; your old address was notified")).toBeInTheDocument();
    expect(requests("POST", "me/email")).toHaveLength(1);
  });

  it("asks for an authenticator code while two-factor authentication is on", async () => {
    signedIn();
    on("GET", "me", me({ totp_enabled: true }));
    on("POST", "me/email", { status: "sent" });
    const { user } = await renderApp("/settings/account/profile");
    await user.click(await screen.findByRole("button", { name: "Change email" }));
    const dialog = await screen.findByRole("dialog", { name: "Change email" });
    const send = within(dialog).getByRole("button", { name: "Send confirmation link" });
    await user.type(within(dialog).getByLabelText("New email"), "ada@example.org");
    expect(send).toBeDisabled();
    await user.type(within(dialog).getByLabelText("Code from the app"), "654321");
    await user.click(send);
    expect(await notifications().findByText("Confirmation link sent; your old address was notified")).toBeInTheDocument();
    expect(requests("POST", "me/email").map((r) => r.body)).toEqual([{ email: "ada@example.org", code: "654321" }]);
  });

  it("sends the confirmation link from its button", async () => {
    signedIn();
    on("GET", "me", me());
    on("POST", "me/email", { status: "sent" });
    const { user } = await renderApp("/settings/account/profile");
    await user.click(await screen.findByRole("button", { name: "Change email" }));
    const dialog = await screen.findByRole("dialog", { name: "Change email" });
    await user.type(within(dialog).getByLabelText("New email"), "ada@example.org");
    await user.click(within(dialog).getByRole("button", { name: "Send confirmation link" }));
    expect(await notifications().findByText("Confirmation link sent; your old address was notified")).toBeInTheDocument();
    expect(requests("POST", "me/email").map((r) => r.body)).toEqual([{ email: "ada@example.org" }]);
  });

  it("opens with an empty address and no error after a refused or cancelled change", async () => {
    signedIn();
    on("GET", "me", me());
    on("POST", "me/email", apiError(409, "conflict", "Another account uses this address.", { param: "email" }));
    const { user } = await renderApp("/settings/account/profile");
    await user.click(await screen.findByRole("button", { name: "Change email" }));
    const dialog = await screen.findByRole("dialog", { name: "Change email" });
    await user.type(within(dialog).getByLabelText("New email"), "grace@example.com");
    await user.click(within(dialog).getByRole("button", { name: "Send confirmation link" }));
    await waitFor(() => expect(within(dialog).getByLabelText("New email")).toHaveAccessibleDescription("Another account uses this address."));
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Change email" })).not.toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Change email" }));
    const again = await screen.findByRole("dialog", { name: "Change email" });
    expect(within(again).getByLabelText("New email")).toHaveValue("");
    expect(within(again).getByLabelText("New email")).not.toHaveAccessibleDescription("Another account uses this address.");
    expect(within(again).getByRole("button", { name: "Send confirmation link" })).toBeDisabled();
  });

  it("marks the address the gateway refused", async () => {
    signedIn();
    on("GET", "me", me());
    on("POST", "me/email", apiError(409, "conflict", "Another account uses this address.", { param: "email" }));
    const { user } = await renderApp("/settings/account/profile");
    await user.click(await screen.findByRole("button", { name: "Change email" }));
    const dialog = await screen.findByRole("dialog", { name: "Change email" });
    await user.type(within(dialog).getByLabelText("New email"), "grace@example.com");
    await user.click(within(dialog).getByRole("button", { name: "Send confirmation link" }));
    await waitFor(() => expect(within(dialog).getByLabelText("New email")).toHaveAccessibleDescription("Another account uses this address."));
  });

  it("says when the profile couldn't load", async () => {
    signedIn();
    on("GET", "me", apiError(429, "rate_limited", "Too many requests; try again in a minute."));
    await renderApp("/settings/account/profile");
    expect(await screen.findByText("Your profile couldn't load")).toBeInTheDocument();
  });

  it("closes the email dialog without sending", async () => {
    signedIn();
    on("GET", "me", me());
    const { user } = await renderApp("/settings/account/profile");
    await user.click(await screen.findByRole("button", { name: "Change email" }));
    const dialog = await screen.findByRole("dialog", { name: "Change email" });
    await user.keyboard("{Enter}");
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(requests("POST", "me/email")).toHaveLength(0);
  });

  it("removes the picture", async () => {
    signedIn();
    on("GET", "me", me({ avatar_url: "/avatars/ada.webp" }));
    on("DELETE", "me/avatar", me());
    const { user } = await renderApp("/settings/account/profile");
    await user.click(await screen.findByRole("button", { name: "Remove" }));
    expect(await notifications().findByText("Picture removed")).toBeInTheDocument();
    expect(requests("DELETE", "me/avatar")).toHaveLength(1);
    expect(screen.getByRole("button", { name: "Replace" })).toBeInTheDocument();
  });

  it("uploads a picture as a square image", async () => {
    signedIn();
    on("GET", "me", me());
    on("PUT", "me/avatar", me({ avatar_url: "/avatars/ada.webp" }));
    vi.stubGlobal("createImageBitmap", async () => ({ width: 400, height: 300, close: () => undefined }));
    const draw = vi.fn();
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({
      drawImage: draw,
      imageSmoothingQuality: "low",
    } as unknown as CanvasRenderingContext2D);
    vi.spyOn(HTMLCanvasElement.prototype, "toBlob").mockImplementation((done) => done(new Blob(["pic"], { type: "image/webp" })));
    const { user } = await renderApp("/settings/account/profile");
    await screen.findByRole("button", { name: "Upload" });
    const input = document.querySelector<HTMLInputElement>('input[type="file"]');
    if (!input) throw new Error("no file input");
    await user.upload(input, new File(["raw"], "ada.png", { type: "image/png" }));
    expect(await notifications().findByText("Picture saved")).toBeInTheDocument();
    expect(requests("PUT", "me/avatar")[0]?.body).toEqual({ content_type: "image/webp", data: btoa("pic") });
    expect(draw).toHaveBeenCalledWith(expect.anything(), 50, 0, 300, 300, 0, 0, 256, 256);
  });

  it("deletes the account once the email is typed", async () => {
    signedIn();
    on("GET", "me", me());
    on("DELETE", "me", noContent);
    const assign = vi.spyOn(window.location, "assign").mockImplementation(() => undefined);
    const { user } = await renderApp("/settings/account/profile");
    await user.click(await screen.findByRole("button", { name: "Delete account" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Delete your account?" });
    const confirm = within(dialog).getByRole("button", { name: "Delete account" });
    expect(confirm).toBeDisabled();
    await user.type(within(dialog).getByRole("textbox"), "owner@example.com");
    await user.click(confirm);
    await waitFor(() => expect(assign).toHaveBeenCalledWith("/sign-in"));
    expect(requests("DELETE", "me")).toHaveLength(1);
  });

  it("deletes an account without an email address without asking to type one", async () => {
    signedIn();
    on("GET", "me", me({ email: null }));
    on("DELETE", "me", noContent);
    const assign = vi.spyOn(window.location, "assign").mockImplementation(() => undefined);
    const { user } = await renderApp("/settings/account/profile");
    await user.click(await screen.findByRole("button", { name: "Delete account" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Delete your account?" });
    expect(within(dialog).queryByRole("textbox")).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Delete account" }));
    await waitFor(() => expect(assign).toHaveBeenCalledWith("/sign-in"));
    expect(requests("DELETE", "me")).toHaveLength(1);
  });

  it("keeps the account and says why when deleting is refused", async () => {
    signedIn();
    on("GET", "me", me());
    on("DELETE", "me", apiError(409, "conflict", "Transfer ownership of Northwind Labs first."));
    const { user } = await renderApp("/settings/account/profile");
    await user.click(await screen.findByRole("button", { name: "Delete account" }));
    const dialog = await screen.findByRole("alertdialog");
    await user.type(within(dialog).getByRole("textbox"), "owner@example.com");
    await user.click(within(dialog).getByRole("button", { name: "Delete account" }));
    expect(await notifications().findByText("Transfer ownership of Northwind Labs first.")).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(screen.getByRole("textbox", { name: "Name" })).toBeInTheDocument();
  });
});
