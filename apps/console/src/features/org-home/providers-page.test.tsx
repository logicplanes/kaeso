import { act, screen, waitFor, within } from "@testing-library/react";
import type { UserEvent } from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CredentialView } from "~/api/generated/types";
import { ORG, OTHER_PROJECT, PROJECT, project, signedIn, withRights } from "~/test/fixtures";
import { OTHER_USER, credential, hoverReason, member, notifications } from "~/test/fixtures/org";
import { showTooltipsAtOnce } from "~/test/tooltip-delay";
import { apiError, noContent, on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { queryClient } from "~/query";

const PATH = `/orgs/${ORG}/providers`;
const CREDENTIALS = "orgs/:org/credentials";

const production = credential({ projects: [{ id: PROJECT, name: "Checkout assistant" }] });
const staging = credential({ id: "cred_staging", name: "Staging", is_default: false, created_at: "2026-10-01T12:00:00Z" });

function keys(list: CredentialView[]) {
  on("GET", CREDENTIALS, { data: list });
}

const table = () => screen.findByRole("table", { name: "Provider keys" });
const wholeText = (text: string) => (_: string, el: Element | null) => el?.textContent === text && ![...el.children].some((c) => c.textContent === text);
const row = async (name: string | RegExp) => (await within(await table()).findByText(name)).closest('[role="row"]') as HTMLElement;

async function openMenu(user: UserEvent, name: string) {
  await user.click(await screen.findByRole("button", { name: `Actions for OpenAI · ${name}` }));
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
        last_used_at: new Date(Date.now() - 3 * 3_600_000).toISOString(),
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
    expect(second).toHaveTextContent("No project");
    expect(second).toHaveTextContent("OpenAI");
    expect(await within(await table()).findByText("Grace Hopper, 15 Oct 2026")).toBeInTheDocument();
    expect(second).toHaveTextContent(/ago/);

    expect(within(await table()).queryByText("Anthropic")).not.toBeInTheDocument();
    expect(within(await table()).queryByText("Groq")).not.toBeInTheDocument();

    const mistral = await row("Mistral main");
    expect(mistral).toHaveTextContent("Mistral");
    expect(mistral).toHaveTextContent("Every unassigned project");
  });

  it("explains provider keys to a new organization and offers to add one", async () => {
    signedIn();
    keys([]);
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("No provider keys yet")).toBeInTheDocument();
    expect(screen.getByText("A provider key lets this organization's projects call that provider's models.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "About provider keys" })).toHaveAttribute("href", "https://docs.kaeso.app/console/provider-keys");
    await user.click(screen.getByRole("button", { name: "Add provider key" }));
    const dialog = await screen.findByRole("dialog", { name: /Add provider key/ });
    expect(within(dialog).getByRole("listbox", { name: "Provider" })).toBeInTheDocument();
    expect(within(dialog).queryByRole("group")).not.toBeInTheDocument();
  });

  it("says why people who can't add keys can't, in an empty organization", async () => {
    withRights(["org.view", "provider_credentials.view"]);
    keys([]);
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("No provider keys yet")).toBeInTheDocument();
    const add = screen.getByRole("button", { name: "Add provider key" });
    await hoverReason(user, add, "You need Add provider keys; ask an owner or admin for it.");
    await user.click(add);
    expect(screen.queryByRole("dialog", { name: /Add provider key/ })).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: "About provider keys" })).toBeInTheDocument();
  });

  it("hides the providers from people who can't view them", async () => {
    withRights(["org.view"]);
    await renderApp(PATH);
    expect(await screen.findByText("You can't see the providers here")).toBeInTheDocument();
    expect(await screen.findByText("You need View provider keys; ask an owner or admin for it.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Add provider key" })).not.toBeInTheDocument();
    expect(requests("GET", CREDENTIALS)).toHaveLength(0);
  });

  it("says when the provider keys couldn't load", async () => {
    signedIn();
    on("GET", CREDENTIALS, apiError(400, "invalid_request", "The credentials store is locked."));
    const { user } = await renderApp(PATH);
    expect(await within(await table()).findByText("Couldn't load these rows")).toBeInTheDocument();
    expect(await notifications().findAllByText("The credentials store is locked.")).not.toHaveLength(0);
    keys([production]);
    await user.click(within(await table()).getByRole("button", { name: "Try again" }));
    expect(await row("OpenAI production")).toBeInTheDocument();
    expect(requests("GET", CREDENTIALS)).toHaveLength(2);
  });

  it("shows the keys without actions to people who can only view them", async () => {
    withRights(["org.view", "provider_credentials.view"]);
    keys([production]);
    const { user } = await renderApp(PATH);
    await row("OpenAI production");
    expect(screen.queryByRole("button", { name: "Actions for OpenAI · OpenAI production" })).not.toBeInTheDocument();
    const add = screen.getByRole("button", { name: "Add provider key" });
    await hoverReason(user, add, "You need Add provider keys; ask an owner or admin for it.");
    await user.click(add);
    expect(screen.queryByRole("dialog", { name: /Add provider key/ })).not.toBeInTheDocument();
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

  it("opens the add and replace dialogs", async () => {
    signedIn();
    keys([production, staging]);
    const { user } = await renderApp(PATH);
    await row("Staging");
    await user.click(screen.getByRole("button", { name: "Add provider key" }));
    expect(await screen.findByRole("dialog", { name: /Add provider key/ })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: /Add provider key/ })).not.toBeInTheDocument());

    const menu = await openMenu(user, "Staging");
    await user.click(within(menu).getByRole("menuitem", { name: "Replace" }));
    expect(await screen.findByRole("dialog", { name: /Replace OpenAI · Staging/ })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: /Replace OpenAI · Staging/ })).not.toBeInTheDocument());

    await user.keyboard("c");
    expect(await screen.findByRole("dialog", { name: /Add provider key/ })).toBeInTheDocument();
  });

  it("shows an account's email and product, and offers to sign in again and manage its usage", async () => {
    signedIn();
    const plan = credential({ id: "cred_plan", name: "Ada's plan", is_default: false, auth_method: "chatgpt", account: "ada@example.com", last4: "" });
    const lapsed = credential({
      id: "cred_lapsed",
      name: "Old plan",
      is_default: false,
      auth_method: "chatgpt",
      account: "grace@example.com",
      last4: "",
      auth_error: "signed_out",
    });
    keys([production, plan, lapsed]);
    const { user } = await renderApp(PATH);

    const account = await row("Ada's plan");
    expect(account).toHaveTextContent("ada@example.com");
    expect(within(account).getByText("ChatGPT")).toBeInTheDocument();
    expect(account).not.toHaveTextContent("…");
    expect(account).not.toHaveTextContent("Signed out");
    expect(await row("Old plan")).toHaveTextContent("Signed out");
    expect(await row("OpenAI production")).toHaveTextContent("…9f3c");

    const menu = await openMenu(user, "Ada's plan");
    expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((i) => i.textContent),
    ).toEqual(["Make default", "Assign to projects", "Sign in again", expect.stringMatching(/^Manage usage/), "Rename", "Delete"]);
    const usage = within(menu).getByRole("menuitem", { name: /Manage usage/ });
    expect(usage).toHaveAttribute("href", "https://chatgpt.com/settings/usage");
    expect(usage).toHaveAttribute("target", "_blank");
    await user.click(within(menu).getByRole("menuitem", { name: "Sign in again" }));
    expect(await screen.findByRole("dialog", { name: /Sign in again to OpenAI · Ada's plan/ })).toBeInTheDocument();
  });

  it("shows a Grok account's email with its product and Experimental badges, and offers to sign in again", async () => {
    signedIn();
    const grok = credential({ id: "cred_grok", name: "Ada's Grok", provider: "xai", auth_method: "grok", account: "ada@example.com", last4: "" });
    const free = credential({
      id: "cred_free",
      name: "Free Grok",
      provider: "xai",
      is_default: false,
      auth_method: "grok",
      account: "grace@example.com",
      last4: "",
      auth_error: "no_api_access",
    });
    keys([production, grok, free]);
    const { user } = await renderApp(PATH);

    const account = await row("Ada's Grok");
    expect(account).toHaveTextContent("ada@example.com");
    expect(within(account).getByText("Grok")).toBeInTheDocument();
    expect(within(account).getByText("Experimental")).toBeInTheDocument();
    expect(account).not.toHaveTextContent("…");
    expect(account).not.toHaveTextContent("No API access");
    const stopped = await row("Free Grok");
    expect(stopped).toHaveTextContent("No API access");
    expect(stopped).not.toHaveTextContent("Signed out");
    await hoverReason(user, within(stopped).getByText("No API access"), "This Grok plan has no API access.");
    expect(within(await row("OpenAI production")).queryByText("Experimental")).not.toBeInTheDocument();

    await user.click(await screen.findByRole("button", { name: "Actions for SpaceX · Ada's Grok" }));
    const menu = await screen.findByRole("menu");
    expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((i) => i.textContent),
    ).toEqual(["Assign to projects", "Sign in again", "Rename", "Delete"]);
    await user.click(within(menu).getByRole("menuitem", { name: "Sign in again" }));
    expect(await screen.findByRole("dialog", { name: /Sign in again to SpaceX · Ada's Grok/ })).toBeInTheDocument();
  });

  it("lets people who can only view keys reach an account's usage", async () => {
    withRights(["org.view", "provider_credentials.view"]);
    keys([production, credential({ id: "cred_plan", name: "Ada's plan", is_default: false, auth_method: "chatgpt", account: "ada@example.com", last4: "" })]);
    const { user } = await renderApp(PATH);
    await row("Ada's plan");
    expect(screen.queryByRole("button", { name: "Actions for OpenAI · OpenAI production" })).not.toBeInTheDocument();
    const menu = await openMenu(user, "Ada's plan");
    expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((i) => i.textContent),
    ).toEqual([expect.stringMatching(/^Manage usage/)]);
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
    const dialog = await screen.findByRole("dialog", { name: "Rename OpenAI · Staging" });
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
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Rename OpenAI · Staging" })).not.toBeInTheDocument());
  });

  it("renames a key with the Rename button", async () => {
    signedIn();
    keys([production, staging]);
    on("PATCH", "orgs/:org/credentials/:credential", ({ body }: { body: { name: string } }) => ({ ...staging, name: body.name }));
    const { user } = await renderApp(PATH);
    await user.click(within(await openMenu(user, "Staging")).getByRole("menuitem", { name: "Rename" }));
    const dialog = await screen.findByRole("dialog", { name: "Rename OpenAI · Staging" });
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
    const dialog = await screen.findByRole("dialog", { name: "Assign OpenAI · Staging" });
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
    const dialog = await screen.findByRole("dialog", { name: "Assign OpenAI · Staging" });
    expect(within(dialog).getByText("These projects use it for OpenAI instead of the default.")).toBeInTheDocument();
    const checkout = await within(dialog).findByRole("checkbox", { name: /Checkout assistant/ });
    expect(within(dialog).getByText("Uses OpenAI production now")).toBeInTheDocument();
    expect(within(dialog).getByRole("checkbox", { name: /Gone project/ })).toBeChecked();
    expect(within(dialog).getByRole("checkbox", { name: /Support triage/ })).not.toBeChecked();
    await user.click(checkout);
    await user.click(within(dialog).getByRole("button", { name: "Save" }));
    expect(await notifications().findByText("OpenAI · Staging assigned to 2 projects")).toBeInTheDocument();
    expect(requests("PUT", "orgs/:org/credentials/:credential/projects").map((r) => r.body)).toEqual([{ projects: ["proj_gone", PROJECT] }]);
  });

  it("says when no project is assigned to a key any more", async () => {
    signedIn();
    keys([production, { ...staging, projects: [{ id: OTHER_PROJECT, name: "Support triage" }] }]);
    on("PUT", "orgs/:org/credentials/:credential/projects", { ...staging, projects: [] });
    const { user } = await renderApp(PATH);
    const menu = await openMenu(user, "Staging");
    await user.click(within(menu).getByRole("menuitem", { name: "Assign to projects" }));
    const dialog = await screen.findByRole("dialog", { name: "Assign OpenAI · Staging" });
    await user.click(await within(dialog).findByRole("checkbox", { name: /Support triage/ }));
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(await notifications().findByText("No project is assigned to OpenAI · Staging")).toBeInTheDocument();
    expect(requests("PUT", "orgs/:org/credentials/:credential/projects").map((r) => r.body)).toEqual([{ projects: [] }]);
  });

  it("closes the rename and assign dialogs without saving", async () => {
    signedIn();
    keys([production, staging]);
    const { user } = await renderApp(PATH);
    await user.click(within(await openMenu(user, "Staging")).getByRole("menuitem", { name: "Rename" }));
    await screen.findByRole("dialog", { name: "Rename OpenAI · Staging" });
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Rename OpenAI · Staging" })).not.toBeInTheDocument());
    await user.click(within(await openMenu(user, "Staging")).getByRole("menuitem", { name: "Assign to projects" }));
    const dialog = await screen.findByRole("dialog", { name: "Assign OpenAI · Staging" });
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Assign OpenAI · Staging" })).not.toBeInTheDocument());
    await user.click(within(await openMenu(user, "Staging")).getByRole("menuitem", { name: "Assign to projects" }));
    await screen.findByRole("dialog", { name: "Assign OpenAI · Staging" });
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Assign OpenAI · Staging" })).not.toBeInTheDocument());
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
      const alert = await screen.findByRole("alertdialog", { name: `Delete OpenAI · ${name}?` });
      expect(within(alert).getByText(text)).toBeInTheDocument();
      if (name !== "Staging") {
        await user.click(within(alert).getByRole("button", { name: "Cancel" }));
        await waitFor(() => expect(screen.queryByRole("alertdialog", { name: `Delete OpenAI · ${name}?` })).not.toBeInTheDocument());
      }
    }
    await user.click(within(screen.getByRole("alertdialog", { name: "Delete OpenAI · Staging?" })).getByRole("button", { name: "Delete key" }));
    expect(await notifications().findByText("OpenAI · Staging deleted")).toBeInTheDocument();
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
    const alert = await screen.findByRole("alertdialog", { name: "Delete OpenAI · OpenAI production?" });
    expect(within(alert).getByText("Requests to OpenAI fail until the organization adds another key.")).toBeInTheDocument();
    await user.click(within(alert).getByRole("button", { name: "Delete key" }));
    expect(await notifications().findAllByText("The key is still in use.")).not.toHaveLength(0);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });

  describe("with many keys", () => {
    const many = [
      ...["openai", "anthropic", "groq", "mistral", "deepseek", "openrouter", "togetherai", "xai"].map((provider, i) =>
        credential({ id: `cred_${provider}`, provider, name: "Demo", projects: i === 0 ? [{ id: OTHER_PROJECT, name: "Support triage" }] : [] }),
      ),
      staging,
    ];

    it("offers no search while the keys fit on a screen", async () => {
      signedIn();
      keys(many.slice(0, 8));
      await renderApp(PATH);
      await row(/Groq/);
      expect(screen.queryByRole("searchbox", { name: "Search provider keys" })).not.toBeInTheDocument();
    });

    it("finds keys by provider, key name or project, and says when none match", async () => {
      signedIn();
      keys(many);
      const { user } = await renderApp(PATH);
      const search = await screen.findByRole("searchbox", { name: "Search provider keys" });
      await user.type(search, "groq");
      await waitFor(() => expect(within(screen.getByRole("table", { name: "Provider keys" })).getAllByRole("row")).toHaveLength(2));
      expect(await row("Groq")).toHaveTextContent("Demo");
      await user.clear(search);
      await user.type(search, "support");
      expect(await row("Demo")).toHaveTextContent("OpenAI");
      expect(within(await table()).queryByText("Staging")).not.toBeInTheDocument();
      await user.clear(search);
      await user.type(search, "staging");
      expect(await row("Staging")).toHaveTextContent("OpenAI");
      await user.clear(search);
      await user.type(search, "nothing like it");
      expect(await screen.findByText("No provider keys match")).toBeInTheDocument();
      expect(screen.getByText("Search by another provider, key or project name.")).toBeInTheDocument();
      await user.click(screen.getByRole("button", { name: "Clear search" }));
      expect(search).toHaveValue("");
      expect(await row("Staging")).toBeInTheDocument();
    });

    it("starts with an empty search when the search comes back after the keys fit again", async () => {
      signedIn();
      keys(many);
      const { user } = await renderApp(PATH);
      await user.type(await screen.findByRole("searchbox", { name: "Search provider keys" }), "groq");
      keys(many.slice(0, 8));
      await act(() => queryClient.invalidateQueries({ queryKey: ["orgs", ORG, "credentials"] }));
      await waitFor(() => expect(screen.queryByRole("searchbox", { name: "Search provider keys" })).not.toBeInTheDocument());
      keys(many);
      await act(() => queryClient.invalidateQueries({ queryKey: ["orgs", ORG, "credentials"] }));
      expect(await screen.findByRole("searchbox", { name: "Search provider keys" })).toHaveValue("");
      expect(await row("Staging")).toBeInTheDocument();
    });

    it("keeps showing the search's result when a later refresh fails", async () => {
      signedIn();
      keys(many);
      const { user } = await renderApp(PATH);
      await user.type(await screen.findByRole("searchbox", { name: "Search provider keys" }), "nothing like it");
      expect(await screen.findByText("No provider keys match")).toBeInTheDocument();
      on("GET", CREDENTIALS, apiError(503, "unavailable", "Down for a moment."));
      await act(() => queryClient.invalidateQueries({ queryKey: ["orgs", ORG, "credentials"] }));
      expect(screen.getByText("No provider keys match")).toBeInTheDocument();
      expect(screen.queryByText("The provider keys couldn't load")).not.toBeInTheDocument();
    });
  });

  it("explains that assigned projects keep the default key when another becomes the default", async () => {
    signedIn();
    keys([production, staging]);
    const { user } = await renderApp(PATH);
    await user.click(within(await openMenu(user, "OpenAI production")).getByRole("menuitem", { name: "Assign to projects" }));
    const dialog = await screen.findByRole("dialog", { name: "Assign OpenAI · OpenAI production" });
    expect(within(dialog).getByText("These projects keep it for OpenAI even when another key becomes the default.")).toBeInTheDocument();
  });

  describe("on a tablet", () => {
    afterEach(() => vi.restoreAllMocks());

    it("shows each key's projects, last use and who added it under its name once their columns give way", async () => {
      vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
        width: 720,
        height: 0,
        top: 0,
        left: 0,
        right: 720,
        bottom: 0,
        x: 0,
        y: 0,
        toJSON: () => ({}),
      });
      signedIn();
      keys([production, { ...staging, projects: [{ id: OTHER_PROJECT, name: "Support triage" }] }]);
      const { user } = await renderApp(PATH);
      await row("Staging");
      const headers = within(await table())
        .getAllByRole("columnheader")
        .map((h) => h.textContent);
      expect(headers).not.toContain("Projects");
      expect(
        await within(await table()).findByText(wholeText("Projects: Support triage · Last used: Never · Added: Ada Okafor (you), 1 Oct 2026")),
      ).toBeInTheDocument();
      expect(
        within(await table()).getByText(
          wholeText("Projects: Checkout assistant, and every unassigned project · Last used: Never · Added: Ada Okafor (you), 15 Oct 2026"),
        ),
      ).toBeInTheDocument();
      act(() => showTooltipsAtOnce(true));
      try {
        await user.hover(within(await table()).getByText(wholeText("Projects: Support triage · Last used: Never · Added: Ada Okafor (you), 1 Oct 2026")));
        expect(await screen.findAllByText("Support triage", { exact: true })).not.toHaveLength(0);
      } finally {
        act(() => showTooltipsAtOnce(false));
      }
    });

    it("shows a tooltip only for keys with assigned projects", async () => {
      signedIn();
      keys([credential({ projects: [] }), { ...staging, projects: [{ id: OTHER_PROJECT, name: "Support triage" }] }]);
      const { user } = await renderApp(PATH);
      await row("Staging");
      act(() => showTooltipsAtOnce(true));
      try {
        await user.hover(within(await table()).getByText("Every unassigned project"));
        await user.hover(within(await table()).getByText("Support triage"));
        await waitFor(() => expect(screen.getAllByText("Support triage", { exact: true }).length).toBeGreaterThan(1));
        expect(within(await table()).getByText("Support triage")).toHaveAttribute("data-popup-open");
        await user.unhover(within(await table()).getByText("Support triage"));
        await user.hover(within(await table()).getByText("Every unassigned project"));
        expect(screen.getAllByText("Every unassigned project", { exact: true })).toHaveLength(1);
        expect(within(await table()).getByText("Every unassigned project")).not.toHaveAttribute("data-popup-open");
      } finally {
        act(() => showTooltipsAtOnce(false));
      }
    });
  });
});
