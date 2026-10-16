import { screen, waitFor, within } from "@testing-library/react";
import type { UserEvent } from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { CredentialView } from "~/api/generated/types";
import { ORG, OTHER_PROJECT, PROJECT, membership, project, signedIn, whoAmI } from "~/test/fixtures";
import { OTHER_USER, credential, member, notifications } from "~/test/fixtures/org";
import { apiError, noContent, on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = `/orgs/${ORG}/providers`;
const CREDENTIALS = "orgs/:org/credentials";

const production = credential({ projects: [{ id: PROJECT, name: "Checkout assistant" }] });
const staging = credential({ id: "cred_staging", name: "Staging", is_default: false, created_at: "2026-10-01T12:00:00Z" });

function keys(list: CredentialView[]) {
  on("GET", CREDENTIALS, { data: list });
}

function withRights(rights: string[]) {
  signedIn({ session: whoAmI({ memberships: [membership({ rights })] }) });
}

const table = () => screen.findByRole("table", { name: "Provider keys" });
const row = async (name: string | RegExp) => (await within(await table()).findByText(name)).closest('[role="row"]') as HTMLElement;

async function openMenu(user: UserEvent, name: string) {
  await user.click(await screen.findByRole("button", { name: `Actions for ${name}` }));
  return screen.findByRole("menu");
}

describe("organization providers", () => {
  it("lists each provider's keys, who added them, the projects they serve and when they were used", async () => {
    signedIn();
    on("GET", "orgs/:org/members", page([member(), member({ user: { id: OTHER_USER, name: "Grace Hopper", email: "grace@example.com" } })]));
    keys([
      credential({
        projects: [
          { id: PROJECT, name: "Checkout assistant" },
          { id: OTHER_PROJECT, name: "Support triage" },
          { id: "proj_three", name: "Billing bot" },
        ],
      }),
      credential({
        id: "cred_staging",
        name: "Staging",
        is_default: false,
        created_by: { kind: "user", id: OTHER_USER },
        last_used_at: "2026-10-15T11:00:00Z",
      }),
      credential({ id: "cred_mistral", name: "Mistral main", provider: "mistral", projects: [] }),
    ]);
    await renderApp(PATH);

    const openai = await row("OpenAI production");
    expect(openai).toHaveTextContent("OpenAI");
    expect(openai).toHaveTextContent("…9f3c");
    expect(openai).toHaveTextContent("Default");
    expect(openai).toHaveTextContent("Checkout assistant, Support triage and 1 more, and every unassigned project");
    expect(openai).toHaveTextContent("Ada Okafor (you), 15 Oct 2026");
    expect(openai).toHaveTextContent("Never");

    const second = await row("Staging");
    expect(second).toHaveTextContent("None");
    expect(await within(await table()).findByText("Grace Hopper, 15 Oct 2026")).toBeInTheDocument();
    expect(second).toHaveTextContent(/ago/);

    const anthropic = await row("Anthropic");
    expect(anthropic).toHaveTextContent("Not connected");
    expect(within(anthropic).getByRole("button", { name: "Connect" })).toBeInTheDocument();

    const mistral = await row("Mistral main");
    expect(mistral).toHaveTextContent("Mistral");
    expect(mistral).toHaveTextContent("Every unassigned project");
  });

  it("hides the providers from people who can't view them", async () => {
    withRights(["org.view"]);
    await renderApp(PATH);
    expect(await screen.findByText("You can't see the providers here")).toBeInTheDocument();
    expect(await screen.findByText("You need View provider keys; ask an owner or admin for it.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Add key" })).not.toBeInTheDocument();
    expect(requests("GET", CREDENTIALS)).toHaveLength(0);
  });

  it("says when the provider keys couldn't load", async () => {
    signedIn();
    on("GET", CREDENTIALS, apiError(400, "invalid_request", "The credentials store is locked."));
    const { user } = await renderApp(PATH);
    expect(await within(await table()).findByText("Couldn’t load these rows")).toBeInTheDocument();
    expect(await notifications().findAllByText("The credentials store is locked.")).not.toHaveLength(0);
    keys([production]);
    await user.click(within(await table()).getByRole("button", { name: "Try again" }));
    expect(await row("OpenAI production")).toBeInTheDocument();
    expect(requests("GET", CREDENTIALS)).toHaveLength(2);
  });

  it("shows the keys without actions to people who can only view them", async () => {
    withRights(["org.view", "provider_credentials.view"]);
    keys([production]);
    await renderApp(PATH);
    await row("OpenAI production");
    expect(screen.queryByRole("button", { name: "Actions for OpenAI production" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Connect" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Add key" })).not.toBeInTheDocument();
  });

  it("offers only deletion to people who can delete but not change keys", async () => {
    withRights(["org.view", "provider_credentials.view", "provider_credentials.delete"]);
    keys([production, staging]);
    const { user } = await renderApp(PATH);
    const menu = await openMenu(user, "Staging");
    expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((i) => i.textContent),
    ).toEqual(["Delete"]);
  });

  it("opens the add, connect and replace dialogs", async () => {
    signedIn();
    keys([production, staging]);
    const { user } = await renderApp(PATH);
    await row("Staging");
    await user.click(screen.getByRole("button", { name: "Add key" }));
    expect(await screen.findByRole("dialog", { name: /Add provider key/ })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: /Add provider key/ })).not.toBeInTheDocument());

    await user.click(within(await row("Anthropic")).getByRole("button", { name: "Connect" }));
    expect(await screen.findByRole("dialog", { name: /Connect Anthropic/ })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: /Connect Anthropic/ })).not.toBeInTheDocument());

    const menu = await openMenu(user, "Staging");
    await user.click(within(menu).getByRole("menuitem", { name: "Replace" }));
    expect(await screen.findByRole("dialog", { name: /Replace Staging/ })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: /Replace Staging/ })).not.toBeInTheDocument());

    await user.keyboard("c");
    expect(await screen.findByRole("dialog", { name: /Add provider key/ })).toBeInTheDocument();
  });

  it("makes a key the provider's default", async () => {
    signedIn();
    keys([production, staging]);
    on("POST", "orgs/:org/credentials/:credential/default", { ...staging, is_default: true });
    const { user } = await renderApp(PATH);
    const defaultMenu = await openMenu(user, "OpenAI production");
    expect(within(defaultMenu).queryByRole("menuitem", { name: "Make default" })).not.toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).not.toBeInTheDocument());
    const menu = await openMenu(user, "Staging");
    await user.click(within(menu).getByRole("menuitem", { name: "Make default" }));
    expect(await notifications().findByText("Staging is the OpenAI default")).toBeInTheDocument();
    expect(requests("POST", "orgs/:org/credentials/:credential/default").map((r) => r.params.credential)).toEqual(["cred_staging"]);
  });

  it("renames a key with the trimmed name", async () => {
    signedIn();
    keys([production, staging]);
    on("PATCH", "orgs/:org/credentials/:credential", ({ body }: { body: { name: string } }) => ({ ...staging, name: body.name }));
    const { user } = await renderApp(PATH);
    const menu = await openMenu(user, "Staging");
    await user.click(within(menu).getByRole("menuitem", { name: "Rename" }));
    const dialog = await screen.findByRole("dialog", { name: "Rename Staging" });
    const input = within(dialog).getByRole("textbox", { name: "Name" });
    expect(input).toHaveValue("Staging");
    await user.hover(within(dialog).getByRole("button", { name: "Rename" }));
    expect(await screen.findByText("Change the name first")).toBeInTheDocument();
    await user.clear(input);
    await user.hover(within(dialog).getByRole("button", { name: "Rename" }));
    expect(await screen.findByText("Give the key a name")).toBeInTheDocument();
    await user.type(input, "  Staging EU  ");
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(await notifications().findByText("Renamed to Staging EU")).toBeInTheDocument();
    expect(requests("PATCH", "orgs/:org/credentials/:credential").map((r) => [r.params.credential, r.body])).toEqual([
      ["cred_staging", { name: "Staging EU" }],
    ]);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Rename Staging" })).not.toBeInTheDocument());
  });

  it("renames a key with the Rename button", async () => {
    signedIn();
    keys([production, staging]);
    on("PATCH", "orgs/:org/credentials/:credential", ({ body }: { body: { name: string } }) => ({ ...staging, name: body.name }));
    const { user } = await renderApp(PATH);
    await user.click(within(await openMenu(user, "Staging")).getByRole("menuitem", { name: "Rename" }));
    const dialog = await screen.findByRole("dialog", { name: "Rename Staging" });
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), " EU");
    await user.click(within(dialog).getByRole("button", { name: "Rename" }));
    expect(await notifications().findByText("Renamed to Staging EU")).toBeInTheDocument();
    expect(requests("PATCH", "orgs/:org/credentials/:credential").map((r) => r.body)).toEqual([{ name: "Staging EU" }]);
  });

  it("lists the projects to assign once they have loaded", async () => {
    signedIn();
    keys([production, { ...staging, projects: [{ id: "proj_gone", name: "Gone project" }] }]);
    const waiting: (() => void)[] = [];
    on("GET", "orgs/:org/projects", () => new Promise((resolve) => waiting.push(() => resolve(page([project()])))));
    const { user } = await renderApp(PATH);
    await user.click(within(await openMenu(user, "Staging")).getByRole("menuitem", { name: "Assign to projects" }));
    const dialog = await screen.findByRole("dialog", { name: "Assign Staging" });
    expect(within(dialog).getByText("Loading projects…")).toBeInTheDocument();
    for (const release of waiting) release();
    expect(await within(dialog).findByRole("checkbox", { name: /Checkout assistant/ })).not.toBeChecked();
    expect(within(dialog).getByRole("checkbox", { name: /Gone project/ })).toBeChecked();
  });

  it("assigns a key to projects and says which ones use another key now", async () => {
    signedIn();
    const assigned = { ...staging, projects: [{ id: "proj_gone", name: "Gone project" }] };
    keys([production, assigned]);
    on("PUT", "orgs/:org/credentials/:credential/projects", ({ body }: { body: { projects: string[] } }) => ({
      ...staging,
      projects: body.projects.map((id) => ({ id, name: id })),
    }));
    const { user } = await renderApp(PATH);
    const menu = await openMenu(user, "Staging");
    await user.click(within(menu).getByRole("menuitem", { name: "Assign to projects" }));
    const dialog = await screen.findByRole("dialog", { name: "Assign Staging" });
    expect(within(dialog).getByText("These projects use it for OpenAI instead of the default.")).toBeInTheDocument();
    const checkout = await within(dialog).findByRole("checkbox", { name: /Checkout assistant/ });
    expect(within(dialog).getByText("Uses OpenAI production now")).toBeInTheDocument();
    expect(within(dialog).getByRole("checkbox", { name: /Gone project/ })).toBeChecked();
    expect(within(dialog).getByRole("checkbox", { name: /Support triage/ })).not.toBeChecked();
    await user.click(checkout);
    await user.click(within(dialog).getByRole("button", { name: "Save" }));
    expect(await notifications().findByText("Staging assigned to 2 projects")).toBeInTheDocument();
    expect(requests("PUT", "orgs/:org/credentials/:credential/projects").map((r) => r.body)).toEqual([{ projects: ["proj_gone", PROJECT] }]);
  });

  it("says when no project is assigned to a key any more", async () => {
    signedIn();
    keys([production, { ...staging, projects: [{ id: OTHER_PROJECT, name: "Support triage" }] }]);
    on("PUT", "orgs/:org/credentials/:credential/projects", { ...staging, projects: [] });
    const { user } = await renderApp(PATH);
    const menu = await openMenu(user, "Staging");
    await user.click(within(menu).getByRole("menuitem", { name: "Assign to projects" }));
    const dialog = await screen.findByRole("dialog", { name: "Assign Staging" });
    await user.click(await within(dialog).findByRole("checkbox", { name: /Support triage/ }));
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(await notifications().findByText("No project is assigned to Staging")).toBeInTheDocument();
    expect(requests("PUT", "orgs/:org/credentials/:credential/projects").map((r) => r.body)).toEqual([{ projects: [] }]);
  });

  it("closes the rename and assign dialogs without saving", async () => {
    signedIn();
    keys([production, staging]);
    const { user } = await renderApp(PATH);
    await user.click(within(await openMenu(user, "Staging")).getByRole("menuitem", { name: "Rename" }));
    await screen.findByRole("dialog", { name: "Rename Staging" });
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Rename Staging" })).not.toBeInTheDocument());
    await user.click(within(await openMenu(user, "Staging")).getByRole("menuitem", { name: "Assign to projects" }));
    const dialog = await screen.findByRole("dialog", { name: "Assign Staging" });
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Assign Staging" })).not.toBeInTheDocument());
    await user.click(within(await openMenu(user, "Staging")).getByRole("menuitem", { name: "Assign to projects" }));
    await screen.findByRole("dialog", { name: "Assign Staging" });
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Assign Staging" })).not.toBeInTheDocument());
    expect(requests("PATCH", "orgs/:org/credentials/:credential")).toHaveLength(0);
    expect(requests("PUT", "orgs/:org/credentials/:credential/projects")).toHaveLength(0);
  });

  it("explains what deleting each key does and deletes it", async () => {
    signedIn();
    const newer = credential({ id: "cred_newer", name: "Newer", is_default: false, created_at: "2026-10-10T12:00:00Z" });
    keys([
      production,
      staging,
      newer,
      credential({
        id: "cred_used",
        name: "Used",
        is_default: false,
        created_at: "2026-09-01T12:00:00Z",
        projects: [{ id: OTHER_PROJECT, name: "Support triage" }],
      }),
    ]);
    on("DELETE", "orgs/:org/credentials/:credential", noContent());
    const { user } = await renderApp(PATH);

    const cases: [string, string][] = [
      ["OpenAI production", "Newer becomes the OpenAI default."],
      ["Used", "Its projects switch to the OpenAI default."],
      ["Staging", "No project uses it; requests are not affected."],
    ];
    for (const [name, text] of cases) {
      const menu = await openMenu(user, name);
      await user.click(within(menu).getByRole("menuitem", { name: "Delete" }));
      const alert = await screen.findByRole("alertdialog", { name: `Delete ${name}?` });
      expect(within(alert).getByText(text)).toBeInTheDocument();
      if (name !== "Staging") {
        await user.click(within(alert).getByRole("button", { name: "Cancel" }));
        await waitFor(() => expect(screen.queryByRole("alertdialog", { name: `Delete ${name}?` })).not.toBeInTheDocument());
      }
    }
    await user.click(within(screen.getByRole("alertdialog", { name: "Delete Staging?" })).getByRole("button", { name: "Delete key" }));
    expect(await notifications().findByText("Staging deleted")).toBeInTheDocument();
    expect(requests("DELETE", "orgs/:org/credentials/:credential").map((r) => r.params.credential)).toEqual(["cred_staging"]);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });

  it("warns that requests fail when the last key goes and closes when the gateway refuses", async () => {
    signedIn();
    keys([production]);
    on("DELETE", "orgs/:org/credentials/:credential", apiError(409, "conflict", "The key is still in use."));
    const { user } = await renderApp(PATH);
    const menu = await openMenu(user, "OpenAI production");
    await user.click(within(menu).getByRole("menuitem", { name: "Delete" }));
    const alert = await screen.findByRole("alertdialog", { name: "Delete OpenAI production?" });
    expect(within(alert).getByText("Requests to OpenAI fail until the organization adds another key.")).toBeInTheDocument();
    await user.click(within(alert).getByRole("button", { name: "Delete key" }));
    expect(await notifications().findAllByText("The key is still in use.")).not.toHaveLength(0);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });
});
