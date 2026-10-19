import { describe, expect, it } from "vitest";
import { screen, within } from "@testing-library/react";
import { apiError, noContent, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const notifications = async () => within(await screen.findByRole("region", { name: "Notifications" }));

describe("cancelling an email change", () => {
  it("asks for the cancel link when the token is missing", async () => {
    await renderApp("/me/email/cancel");
    expect(await screen.findByText("This page needs the cancel link")).toBeInTheDocument();
    expect(requests("POST", "me/email/cancel")).toHaveLength(0);
  });

  it("cancels the change once and says the address stays", async () => {
    on("POST", "me/email/cancel", noContent);
    await renderApp("/me/email/cancel#token=ecc_1");
    expect(await screen.findByRole("heading", { name: "The email change is cancelled", level: 1 })).toBeInTheDocument();
    expect(window.location.hash).toBe("");
    expect(requests("POST", "me/email/cancel").map((r) => r.body)).toEqual([{ token: "ecc_1" }]);
    expect(screen.getByRole("link", { name: "Go to sign in" })).toHaveAttribute("href", "/sign-in");
  });

  it("says when the link no longer works", async () => {
    on("POST", "me/email/cancel", apiError(401, "authentication", "This link is invalid, used or expired; the change may already be done."));
    await renderApp("/me/email/cancel#token=ecc_1");
    expect(await screen.findByText("This link has expired or was already used.")).toBeInTheDocument();
    expect((await (await notifications()).findAllByText("This link is invalid, used or expired; the change may already be done.")).length).toBeGreaterThan(0);
  });
});
