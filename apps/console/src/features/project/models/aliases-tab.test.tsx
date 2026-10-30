import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { signedIn, without } from "~/test/fixtures";
import { hoverReason, notifications } from "~/test/fixtures/org";
import { alias, catalogueModel, modelsApi, projectData, projectPath } from "~/test/fixtures/project";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const ALIASES = "orgs/:org/projects/:project/aliases";
const ONE_ALIAS = "orgs/:org/projects/:project/aliases/:alias";

const models = () => [
  catalogueModel(),
  catalogueModel({ id: "cm_haiku", model_id: "claude-haiku-4-5", effective: { ...catalogueModel().effective, display_name: "claude-haiku-4-5" } }),
];

const aliases = () => [
  alias(),
  alias({ id: "al_haiku", name: "claude-haiku-4-5", catalogue_model_id: "cm_haiku", target: "anthropic/claude-haiku-4-5", shadows_model: true }),
];

function setUp({ rights, list = aliases() }: { rights?: string[]; list?: ReturnType<typeof aliases> } = {}) {
  signedIn(rights ? { projectRights: rights } : {});
  projectData({ aliases: list });
  modelsApi({ models: models() });
}

describe("AliasesTab", () => {
  it("lists each alias with its target and warns when it shadows a model", async () => {
    setUp();
    const { user } = await renderApp(projectPath("models?tab=aliases"));

    const fast = await screen.findByRole("gridcell", { name: /^fast/ });
    expect(fast).toHaveTextContent("→ Claude Sonnet 4.5anthropic/claude-sonnet-4-5");
    const haiku = screen.getByRole("gridcell", { name: /^claude-haiku-4-5/ });
    expect(haiku).toHaveTextContent("anthropic/claude-haiku-4-5");
    expect(haiku).not.toHaveTextContent("→ claude-haiku-4-5");
    await hoverReason(user, within(haiku).getByText("Shadows a model"), "Apps that send `claude-haiku-4-5` get this alias, not the model with the same name.");
  });

  it("says the aliases couldn't load instead of offering the first one", async () => {
    setUp();
    on("GET", ALIASES, apiError(503, "unavailable", "Kaeso can't reach its database right now; try again shortly."));
    await renderApp(projectPath("models?tab=aliases"));

    expect(await screen.findByText("The aliases couldn't load")).toBeInTheDocument();
    expect(screen.queryByText("No aliases yet")).not.toBeInTheDocument();
  });

  it("offers to create the first alias", async () => {
    setUp({ list: [] });
    const { user } = await renderApp(projectPath("models?tab=aliases"));

    expect(await screen.findByText("No aliases yet")).toBeInTheDocument();
    expect(screen.getByText("fast", { selector: "code" }).parentElement).toHaveTextContent(
      /^A name like fast that apps send as model; retarget it and all apps follow\.$/,
    );
    expect(screen.getByText("model", { selector: "code" })).toBeInTheDocument();
    const create = screen.getAllByRole("button", { name: "Create alias" });
    expect(create).toHaveLength(2);
    await user.click(create[1] as HTMLElement);
    const dialog = await screen.findByRole("dialog", { name: "Create alias" });
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Create alias" })).not.toBeInTheDocument());
    await user.click(screen.getAllByRole("button", { name: "Create alias" })[1] as HTMLElement);
    await screen.findByRole("dialog", { name: "Create alias" });
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Create alias" })).not.toBeInTheDocument());
  });

  it("creates an alias and warns when its name is a model's", async () => {
    setUp();
    on("POST", ALIASES, ({ body }: { body: { name: string; catalogue_model_id: string } }) => alias({ id: "al_new", ...body }));
    const { user } = await renderApp(projectPath("models?tab=aliases&add=true"));

    const dialog = await screen.findByRole("dialog", { name: "Create alias" });
    const submit = within(dialog).getByRole("button", { name: "Create alias" });
    expect(submit).toBeDisabled();
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    const name = within(dialog).getByRole("textbox", { name: "Name" });
    await user.type(name, "claude-sonnet-4-5");
    expect(within(dialog).getByText("Apps that send `claude-sonnet-4-5` get this alias, not the model with the same name.")).toBeInTheDocument();
    await user.clear(name);
    await user.type(name, " smart ");
    await user.click(within(dialog).getByRole("button", { name: "Target" }));
    await user.click(await screen.findByRole("option", { name: /claude-haiku-4-5/ }));
    await user.keyboard("{Meta>}{Enter}{/Meta}");

    expect(await notifications().findByText("Alias smart created")).toBeInTheDocument();
    expect(requests("POST", ALIASES).map((r) => r.body)).toEqual([{ name: "smart", catalogue_model_id: "cm_haiku" }]);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Create alias" })).not.toBeInTheDocument());
  });

  it("shows the API's message when the alias is refused", async () => {
    setUp();
    on("POST", ALIASES, apiError(409, "conflict", "An alias with that name exists."));
    const { user } = await renderApp(projectPath("models?tab=aliases"));

    await screen.findByRole("gridcell", { name: /^fast/ });
    await user.click(screen.getByRole("button", { name: "Create alias" }));
    const dialog = await screen.findByRole("dialog", { name: "Create alias" });
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "fast");
    await user.click(within(dialog).getByRole("button", { name: "Target" }));
    await user.click(await screen.findByRole("option", { name: /Claude Sonnet 4.5/ }));
    await user.click(within(dialog).getByRole("button", { name: "Create alias" }));
    expect((await notifications().findAllByText("An alias with that name exists.")).length).toBeGreaterThan(0);
  });

  it("retargets an alias opened from its row, sending only what changed", async () => {
    setUp();
    on("PATCH", ONE_ALIAS, ({ body }: { body: Record<string, string> }) => alias({ ...body }));
    const { user } = await renderApp(projectPath("models?tab=aliases"));

    await user.click(await screen.findByRole("gridcell", { name: /^fast/ }));
    const dialog = await screen.findByRole("dialog", { name: "Edit fast" });
    expect(within(dialog).getByRole("button", { name: "Save alias" })).toBeDisabled();
    await user.click(within(dialog).getByRole("button", { name: "Target" }));
    await user.click(await screen.findByRole("option", { name: /claude-haiku-4-5/ }));
    await user.click(within(dialog).getByRole("button", { name: "Save alias" }));

    expect(await notifications().findByText("Alias fast saved")).toBeInTheDocument();
    expect(requests("PATCH", ONE_ALIAS).map((r) => [r.params.alias, r.body])).toEqual([["al_fast", { catalogue_model_id: "cm_haiku" }]]);
  });

  it("renames an alias from its menu", async () => {
    setUp();
    on("PATCH", ONE_ALIAS, ({ body }: { body: Record<string, string> }) => alias({ ...body }));
    const { user } = await renderApp(projectPath("models?tab=aliases"));

    await user.click(await screen.findByRole("button", { name: "Actions for fast" }));
    await user.click(await screen.findByRole("menuitem", { name: "Edit" }));
    const dialog = await screen.findByRole("dialog", { name: "Edit fast" });
    const name = within(dialog).getByRole("textbox", { name: "Name" });
    await user.clear(name);
    await user.type(name, "quick");
    await user.click(within(dialog).getByRole("button", { name: "Save alias" }));

    expect(await notifications().findByText("Alias quick saved")).toBeInTheDocument();
    expect(requests("PATCH", ONE_ALIAS).map((r) => r.body)).toEqual([{ name: "quick" }]);
  });

  it("deletes an alias after confirming", async () => {
    setUp();
    on("DELETE", ONE_ALIAS, () => undefined);
    const { user } = await renderApp(projectPath("models?tab=aliases"));

    await user.click(await screen.findByRole("button", { name: "Actions for fast" }));
    await user.click(await screen.findByRole("menuitem", { name: "Delete" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Delete the alias fast?" });
    expect(within(confirm).getByText("Requests that send this name are refused, unless a model has the same name.")).toBeInTheDocument();
    await user.click(within(confirm).getByRole("button", { name: "Delete alias" }));

    expect(await notifications().findByText("Alias fast deleted")).toBeInTheDocument();
    expect(requests("DELETE", ONE_ALIAS).map((r) => r.params.alias)).toEqual(["al_fast"]);
    await waitFor(() => expect(screen.queryByRole("alertdialog", { name: "Delete the alias fast?" })).not.toBeInTheDocument());
  });

  it("shows aliases read-only without Manage models", async () => {
    setUp({ rights: without("models.manage") });
    const { user } = await renderApp(projectPath("models?tab=aliases"));

    await user.click(await screen.findByRole("gridcell", { name: /^fast/ }));
    expect(screen.queryByRole("dialog", { name: "Edit fast" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Actions for fast" })).not.toBeInTheDocument();
    const create = screen.getByRole("button", { name: "Create alias" });
    expect(create).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, create, /You need Manage models/);
  });

  it("offers no create button in the empty state without Manage models", async () => {
    setUp({ rights: without("models.manage"), list: [] });
    await renderApp(projectPath("models?tab=aliases"));

    expect(await screen.findByText("No aliases yet")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "Create alias" })).toHaveLength(1);
  });
});
