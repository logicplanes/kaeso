import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { ORG, signedIn } from "~/test/fixtures";
import { signedInReply } from "~/test/fixtures/public";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const notifications = async () => within(await screen.findByRole("region", { name: "Notifications" }));

async function fill(user: Awaited<ReturnType<typeof renderApp>>["user"], password = "correct horse battery") {
  await user.type(await screen.findByRole("textbox", { name: "Name" }), "  Ada Okafor ");
  await user.type(screen.getByRole("textbox", { name: "Email" }), " owner@example.com ");
  await user.type(screen.getByLabelText("Password"), password);
  await user.type(screen.getByRole("textbox", { name: "Organization name" }), " Northwind Labs ");
}

describe("setting up Kaeso", () => {
  it("asks for the setup link when the page opens without one", async () => {
    await renderApp("/setup");
    expect(await screen.findByText("This page needs the setup link")).toBeInTheDocument();
    expect(screen.getByText("Open the setup link from the gateway's log; it ends with #token=…")).toBeInTheDocument();
    expect(screen.getByText("kaeso setup-link")).toBeInTheDocument();
    expect(screen.queryByRole("textbox", { name: "Name" })).not.toBeInTheDocument();
  });

  it("takes the token from the fragment and drops it from the address", async () => {
    await renderApp("/setup#token=stp_1");
    expect(await screen.findByText("Create the first user, an instance admin, and the organization they own.")).toBeInTheDocument();
    expect(window.location.hash).toBe("");
    expect(window.location.pathname).toBe("/setup");
  });

  it("guides the password and creates the first user and organization", async () => {
    const { user } = await renderApp("/setup#token=stp_1");
    expect(await screen.findByText("12 to 128 characters. A few unrelated words are easy to remember and hard to guess.")).toBeInTheDocument();
    const submit = screen.getByRole("button", { name: "Set up Kaeso" });
    expect(submit).toBeDisabled();
    await user.type(screen.getByLabelText("Password"), "short");
    expect(screen.getByText("7 more characters to go.")).toBeInTheDocument();
    await user.type(screen.getByLabelText("Password"), "1234567");
    expect(screen.getByText("Long enough.")).toBeInTheDocument();
    await user.clear(screen.getByLabelText("Password"));
    await user.type(screen.getByLabelText("Password"), "x".repeat(11));
    expect(screen.getByText("1 more character to go.")).toBeInTheDocument();
    await user.clear(screen.getByLabelText("Password"));
    await fill(user);
    on("POST", "auth/setup", () => {
      signedIn();
      return signedInReply();
    });
    await user.click(screen.getByRole("button", { name: "Set up Kaeso" }));
    await waitFor(() => expect(window.location.pathname).toMatch(new RegExp(`^/orgs/${ORG}`)));
    expect(requests("POST", "auth/setup").map((r) => r.body)).toEqual([
      { token: "stp_1", name: "Ada Okafor", email: "owner@example.com", password: "correct horse battery", org_name: "Northwind Labs" },
    ]);
  });

  it("refuses a password longer than 128 characters", async () => {
    const { user } = await renderApp("/setup#token=stp_1");
    await screen.findByRole("textbox", { name: "Name" });
    const field = screen.getByLabelText("Password");
    field.focus();
    await user.paste("x".repeat(129));
    expect(screen.getByText("At most 128 characters.")).toBeInTheDocument();
    await user.type(screen.getByRole("textbox", { name: "Name" }), "Ada{Enter}");
    expect(requests("POST", "auth/setup")).toHaveLength(0);
  });

  it("shows a field problem and keeps the form", async () => {
    on("POST", "auth/setup", apiError(422, "invalid_request", "Enter a valid email address.", { param: "email" }));
    const { user } = await renderApp("/setup#token=stp_1");
    await fill(user);
    await user.type(screen.getByRole("textbox", { name: "Organization name" }), "{Enter}");
    expect((await (await notifications()).findAllByText("Enter a valid email address.")).length).toBeGreaterThan(0);
    expect(screen.getByRole("textbox", { name: "Name" })).toHaveValue("  Ada Okafor ");
    expect(screen.getByRole("button", { name: "Set up Kaeso" })).toBeEnabled();
  });

  it("explains a used setup link", async () => {
    on("POST", "auth/setup", apiError(409, "conflict", "Kaeso is already set up."));
    const { user } = await renderApp("/setup#token=stp_1");
    await fill(user);
    await user.click(screen.getByRole("button", { name: "Set up Kaeso" }));
    expect(await screen.findByText("This setup link can't be used")).toBeInTheDocument();
    expect((await (await notifications()).findAllByText("Kaeso is already set up.")).length).toBeGreaterThan(0);
    expect(screen.queryByRole("textbox", { name: "Name" })).not.toBeInTheDocument();
  });

  it("explains a wrong setup token", async () => {
    on("POST", "auth/setup", apiError(422, "invalid_request", "That setup token is wrong.", { param: "token" }));
    const { user } = await renderApp("/setup#token=stp_1");
    await fill(user);
    await user.click(screen.getByRole("button", { name: "Set up Kaeso" }));
    expect(await screen.findByText("This setup link can't be used")).toBeInTheDocument();
  });

  it("says when the gateway can't be reached", async () => {
    on("POST", "auth/setup", () => {
      throw new TypeError("Failed to fetch");
    });
    const { user } = await renderApp("/setup#token=stp_1");
    await fill(user);
    await user.click(screen.getByRole("button", { name: "Set up Kaeso" }));
    expect((await (await notifications()).findAllByText("Can't reach the gateway. Try again.")).length).toBeGreaterThan(0);
    expect(screen.getByRole("textbox", { name: "Organization name" })).toBeInTheDocument();
  });
});
