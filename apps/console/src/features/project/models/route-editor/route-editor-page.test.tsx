import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ORG, PROJECT, USER, signedIn, without } from "~/test/fixtures";
import { notifications } from "~/test/fixtures/org";
import { alias, apiKey, catalogueModel, modelRoute, modelsApi, projectData, projectPath } from "~/test/fixtures/project";
import { apiError, on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const ALIASES = "orgs/:org/projects/:project/aliases";
const ONE_ALIAS = "orgs/:org/projects/:project/aliases/:alias";
const AUDIT = "orgs/:org/projects/:project/audit-log";
const TRACE = "orgs/:org/projects/:project/aliases/trace";

type Traced = {
  route: unknown;
  alias: string | null;
  request: {
    headers: { name: string; value: string }[];
    format: string;
    features: string[];
    input_tokens: number;
    key: string | null;
    member: string | null;
    seed: number;
  };
};

const planned = () =>
  alias({
    route: {
      type: "condition",
      match: "all",
      rules: [{ field: "header", name: "x-plan", op: "equals", value: "enterprise" }],
      then: modelRoute("cm_sonnet", modelRoute("cm_haiku")),
      else: {
        type: "split",
        by: "request",
        branches: [
          { weight: 50, step: modelRoute("cm_sonnet") },
          { weight: 50, step: modelRoute("cm_haiku") },
        ],
      },
    },
  });

const models = () => [
  catalogueModel(),
  catalogueModel({ id: "cm_haiku", model_id: "claude-haiku-4-5", effective: { ...catalogueModel().effective, display_name: "Claude Haiku 4.5" } }),
];

const sturdy = () =>
  alias({
    route: modelRoute("cm_sonnet", modelRoute("cm_haiku")),
    models: [
      { catalogue_model_id: "cm_sonnet", target: "anthropic/claude-sonnet-4-5" },
      { catalogue_model_id: "cm_haiku", target: "anthropic/claude-haiku-4-5" },
    ],
  });

function setUp({ rights, one = sturdy() }: { rights?: string[]; one?: ReturnType<typeof alias> } = {}) {
  signedIn(rights ? { projectRights: rights } : {});
  projectData({ aliases: [one], models: models() });
  modelsApi({ models: models() });
  on("GET", ONE_ALIAS, one);
}

const KIND = "^(?:Model|Condition|Split|Start)";
const stepName = (name: RegExp) => new RegExp(`${KIND}.*${name.source}`);
const step = (name: RegExp) => screen.findByRole("treeitem", { name: stepName(name) });

describe("RouteEditorPage", () => {
  it("draws the route and saves a change to when a step falls back", async () => {
    setUp();
    on("PATCH", ONE_ALIAS, ({ body }: { body: { route: unknown } }) => ({ ...sturdy(), route: body.route }));
    const { user } = await renderApp(projectPath("models/aliases/al_fast"));

    expect(await screen.findByRole("heading", { name: "fast" })).toBeInTheDocument();
    expect(await step(/Claude Haiku 4\.5/)).toBeInTheDocument();
    const save = screen.getByRole("button", { name: "Save" });
    expect(save).toHaveAttribute("aria-disabled", "true");

    await user.click(await step(/Claude Sonnet 4\.5/));
    const panel = await screen.findByRole("complementary", { name: "Step settings" });
    expect(within(panel).getByRole("heading", { name: "Model" })).toBeInTheDocument();
    await user.click(within(panel).getByRole("checkbox", { name: /The provider refuses the request/ }));
    expect(save).not.toHaveAttribute("aria-disabled");
    await user.click(save);

    expect(await notifications().findByText("Alias fast saved")).toBeInTheDocument();
    const [body] = requests("PATCH", ONE_ALIAS).map((r) => r.body as { route: { fall_back_on: string[]; fallback: { model: string } } });
    expect(body?.route.fall_back_on).toEqual(["unavailable", "limited", "unsupported", "rejected"]);
    expect(body?.route.fallback.model).toBe("cm_haiku");
  });

  it("adds a fallback on the canvas and asks for its model before saving", async () => {
    setUp({ one: alias() });
    on("PATCH", ONE_ALIAS, ({ body }: { body: { route: unknown } }) => ({ ...alias(), route: body.route }));
    const { user } = await renderApp(projectPath("models/aliases/al_fast"));

    await user.click(await screen.findByRole("treeitem", { name: "Add a fallback to Claude Sonnet 4.5" }));
    await user.click(await screen.findByRole("menuitem", { name: "Fall back to a model" }));
    expect(await step(/No model yet/)).toHaveTextContent("Choose a model");
    const save = screen.getByRole("button", { name: "Save" });
    expect(save).toHaveAttribute("aria-disabled", "true");

    const panel = screen.getByRole("complementary", { name: "Step settings" });
    await user.click(within(panel).getByRole("button", { name: /^Model/ }));
    await user.click(await screen.findByRole("option", { name: /Claude Haiku 4\.5/ }));
    await waitFor(() => expect(save).not.toHaveAttribute("aria-disabled"));
    await user.click(save);

    expect(await notifications().findByText("Alias fast saved")).toBeInTheDocument();
    const [body] = requests("PATCH", ONE_ALIAS).map((r) => r.body as { route: { model: string; fallback: { model: string } } });
    expect(body?.route.model).toBe("cm_sonnet");
    expect(body?.route.fallback.model).toBe("cm_haiku");
  });

  it("creates an alias with a split and opens its page", async () => {
    signedIn();
    projectData({ aliases: [], models: models() });
    modelsApi({ models: models() });
    on("POST", ALIASES, ({ body }: { body: { name: string; route: unknown } }) => ({ ...alias({ id: "al_ab", name: body.name }), route: body.route }));
    on("GET", ONE_ALIAS, alias({ id: "al_ab", name: "ab" }));
    const { user } = await renderApp(projectPath("models/aliases/new"));

    expect(await screen.findByRole("heading", { name: "New alias" })).toBeInTheDocument();
    await user.type(screen.getByRole("textbox", { name: "Name" }), "ab");
    await user.click(await step(/No model yet/));
    const panel = screen.getByRole("complementary", { name: "Step settings" });
    await user.click(within(panel).getByRole("button", { name: /^Model/ }));
    await user.click(await screen.findByRole("option", { name: /Claude Sonnet 4\.5/ }));

    await user.click(screen.getByRole("treeitem", { name: stepName(/Claude Sonnet 4\.5/) }));
    await user.keyboard("{Shift>}{F10}{/Shift}");
    await user.click(await screen.findByRole("menuitem", { name: "Split the traffic" }));
    expect(await step(/Per request/)).toBeInTheDocument();
    const split = screen.getByRole("complementary", { name: "Step settings" });
    expect(within(split).getByRole("heading", { name: "Split" })).toBeInTheDocument();
    await user.click(within(split).getByRole("radio", { name: "Per caller" }));
    await user.click(await step(/No model yet/));
    const second = screen.getByRole("complementary", { name: "Step settings" });
    await user.click(within(second).getByRole("button", { name: /^Model/ }));
    await user.click(await screen.findByRole("option", { name: /Claude Haiku 4\.5/ }));
    await user.click(screen.getByRole("button", { name: "Create alias" }));

    expect(await notifications().findByText("Alias ab created")).toBeInTheDocument();
    expect(requests("POST", ALIASES).map((r) => r.body)).toEqual([
      {
        name: "ab",
        route: {
          type: "split",
          by: "caller",
          branches: [
            { weight: 50, step: modelRoute("cm_sonnet") },
            { weight: 50, step: modelRoute("cm_haiku") },
          ],
        },
      },
    ]);
    await waitFor(() => expect(window.location.pathname).toBe(`/orgs/${ORG}/projects/${PROJECT}/models/aliases/al_ab`));
  });

  it("puts a refused field on its step", async () => {
    setUp();
    on("PATCH", ONE_ALIAS, apiError(400, "invalid_request", "This model is not in the project's catalogue.", { param: "route.fallback.model" }));
    const { user } = await renderApp(projectPath("models/aliases/al_fast"));

    await user.click(await step(/Claude Haiku 4\.5/));
    const panel = screen.getByRole("complementary", { name: "Step settings" });
    await user.click(within(panel).getByRole("checkbox", { name: /The provider refuses the request/ }));
    await user.click(screen.getByRole("button", { name: "Save" }));
    const refused = await screen.findAllByText("This model is not in the project's catalogue.");
    expect(refused.length).toBeGreaterThan(0);
    expect(screen.getByRole("treeitem", { name: stepName(/Claude Haiku 4\.5/) })).toHaveAttribute("aria-selected", "true");
  });

  it("undoes and redoes changes", async () => {
    setUp();
    const { user } = await renderApp(projectPath("models/aliases/al_fast"));

    await user.click(await step(/Claude Haiku 4\.5/));
    await user.click(screen.getByRole("button", { name: "Remove step" }));
    await waitFor(() => expect(screen.queryByRole("treeitem", { name: stepName(/Claude Haiku 4\.5/) })).not.toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Undo" }));
    expect(await step(/Claude Haiku 4\.5/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Redo" }));
    await waitFor(() => expect(screen.queryByRole("treeitem", { name: stepName(/Claude Haiku 4\.5/) })).not.toBeInTheDocument());
  });

  it("asks before leaving with unsaved changes", async () => {
    setUp();
    const { user } = await renderApp(projectPath("models/aliases/al_fast"));

    await user.click(await step(/Claude Haiku 4\.5/));
    await user.click(screen.getByRole("button", { name: "Remove step" }));
    await user.click(screen.getByRole("button", { name: "Back to aliases" }));
    const ask = await screen.findByRole("alertdialog", { name: "Leave without saving?" });
    await user.click(within(ask).getByRole("button", { name: "Leave" }));
    await waitFor(() => expect(window.location.pathname).toBe(`/orgs/${ORG}/projects/${PROJECT}/models`));
  });

  it("shows the route as JSON", async () => {
    setUp();
    const { user } = await renderApp(projectPath("models/aliases/al_fast"));

    await user.click(await screen.findByRole("button", { name: "Show as JSON" }));
    const dialog = await screen.findByRole("dialog", { name: "The route as JSON" });
    expect(dialog).toHaveTextContent('"model": "cm_haiku"');
  });

  it("lists versions and restores one", async () => {
    setUp();
    on("GET", AUDIT, ({ query }: { query: URLSearchParams }) =>
      query.get("target") === "al_fast"
        ? page([
            {
              id: "evt_2",
              at: "2026-11-03T10:00:00Z",
              action: "alias.update",
              actor: { kind: "user", id: USER },
              after: { name: "fast", route: sturdy().route },
            },
            {
              id: "evt_15",
              at: "2026-11-02T10:00:00Z",
              action: "alias.update",
              actor: { kind: "user", id: USER },
              after: { name: "fast" },
            },
            {
              id: "evt_12",
              at: "2026-11-02T09:00:00Z",
              action: "alias.update",
              actor: { kind: "service_account", id: "sa_deploy" },
              after: { name: "fast", route: modelRoute("cm_sonnet") },
            },
            {
              id: "evt_1",
              at: "2026-11-01T10:00:00Z",
              action: "alias.create",
              actor: { kind: "user", id: USER },
              after: { name: "fast", catalogue_model_id: "cm_haiku" },
            },
          ])
        : page([]),
    );
    const { user } = await renderApp(projectPath("models/aliases/al_fast"));

    await user.click(await screen.findByRole("tab", { name: "Versions" }));
    const created = await screen.findByRole("gridcell", { name: /^Created by/ });
    await user.click(created);
    expect(await step(/Claude Haiku 4\.5/)).toBeInTheDocument();
    expect(screen.queryByRole("treeitem", { name: stepName(/Claude Sonnet 4\.5/) })).not.toBeInTheDocument();
    await user.click(await step(/Claude Haiku 4\.5/));
    expect(screen.queryByRole("heading", { name: "Model" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("gridcell", { name: /^Created by/ }));
    expect(await step(/Claude Sonnet 4\.5/)).toBeInTheDocument();
    await user.click(screen.getByRole("gridcell", { name: /^Saved by .*Live/ }));
    expect(await screen.findByText("It routes like your route does.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Restore this version" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Show your route" }));
    expect(screen.queryByText("It routes like your route does.")).not.toBeInTheDocument();
    await user.click(screen.getByRole("gridcell", { name: /^Created by/ }));
    expect(await screen.findByText("Your route differs; restore this version to edit it.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Changes as JSON" }));
    expect(await screen.findByText("This version → your route")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Restore this version" }));
    expect(await notifications().findByText("Version restored; save to make it live")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save" })).not.toHaveAttribute("aria-disabled");
  });

  it("shows the route read-only without Manage models", async () => {
    setUp({ rights: without("models.manage") });
    const { user } = await renderApp(projectPath("models/aliases/al_fast"));

    await user.click(await step(/Claude Sonnet 4\.5/));
    expect(screen.queryByRole("button", { name: "Save" })).not.toBeInTheDocument();
    expect(screen.queryByRole("treeitem", { name: stepName(/Add a fallback/) })).not.toBeInTheDocument();
    const panel = screen.getByRole("complementary", { name: "Step settings" });
    expect(within(panel).getByRole("checkbox", { name: /The provider is unavailable/ })).toHaveAttribute("aria-disabled", "true");
  });

  it("says when the alias or the catalogue couldn't load", async () => {
    setUp();
    on("GET", ONE_ALIAS, apiError(500, "internal", "Something went wrong."));
    await renderApp(projectPath("models/aliases/al_fast"));
    expect(await screen.findByText("This alias couldn't load")).toBeInTheDocument();
  });

  it("says when the catalogue couldn't load for a new alias", async () => {
    setUp();
    on("GET", "orgs/:org/projects/:project/models", apiError(500, "internal", "Something went wrong."));
    await renderApp(projectPath("models/aliases/new"));
    expect(await screen.findByText("The catalogue couldn't load")).toBeInTheDocument();
  });

  it("undoes, redoes and saves from the keyboard, but leaves typing alone", async () => {
    setUp();
    on("PATCH", ONE_ALIAS, ({ body }: { body: { name: string; route: unknown } }) => ({ ...sturdy(), ...body }));
    const { user } = await renderApp(projectPath("models/aliases/al_fast"));

    await user.click(await step(/Claude Haiku 4\.5/));
    await user.click(screen.getByRole("button", { name: "Remove step" }));
    await waitFor(() => expect(screen.queryByRole("treeitem", { name: stepName(/Claude Haiku 4\.5/) })).not.toBeInTheDocument());
    await user.keyboard("{Control>}z{/Control}");
    expect(await step(/Claude Haiku 4\.5/)).toBeInTheDocument();
    await user.keyboard("{Control>}{Shift>}z{/Shift}{/Control}");
    await waitFor(() => expect(screen.queryByRole("treeitem", { name: stepName(/Claude Haiku 4\.5/) })).not.toBeInTheDocument());
    await user.keyboard("{Control>}z{/Control}");
    await user.keyboard("{Control>}y{/Control}");
    await waitFor(() => expect(screen.queryByRole("treeitem", { name: stepName(/Claude Haiku 4\.5/) })).not.toBeInTheDocument());
    await user.keyboard("z");

    await user.click(screen.getByRole("treeitem", { name: /^Start/ }));
    const name = screen.getByRole("textbox", { name: "Name" });
    await user.type(name, "x");
    await user.keyboard("{Control>}z{/Control}");
    await waitFor(() => expect(screen.queryByRole("treeitem", { name: stepName(/Claude Haiku 4\.5/) })).not.toBeInTheDocument());
    await user.keyboard("{Control>}s{/Control}");
    expect(await notifications().findByText("Alias fastx saved")).toBeInTheDocument();
    expect(requests("PATCH", ONE_ALIAS).map((r) => Object.keys(r.body as object).sort())).toEqual([["name", "route"]]);
  });

  it("puts a refused name under the name", async () => {
    setUp();
    on("PATCH", ONE_ALIAS, apiError(400, "invalid_request", "An alias must not contain spaces, control characters or `/`.", { param: "name" }));
    const { user } = await renderApp(projectPath("models/aliases/al_fast"));

    await user.click(await screen.findByRole("treeitem", { name: /^Start/ }));
    const name = screen.getByRole("textbox", { name: "Name" });
    await user.type(name, "/x");
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(name).toHaveAccessibleDescription("An alias must not contain spaces, control characters or `/`."));
    await user.type(name, "y");
    expect(name).not.toHaveAccessibleDescription("An alias must not contain spaces, control characters or `/`.");
  });

  it("keeps a refusal it can't place in a toast", async () => {
    setUp();
    on("PATCH", ONE_ALIAS, apiError(400, "invalid_request", "A route has at most 32 steps.", { param: "route.fallback.fallback" }));
    const { user } = await renderApp(projectPath("models/aliases/al_fast"));

    await user.click(await step(/Claude Haiku 4\.5/));
    await user.click(within(screen.getByRole("complementary", { name: "Step settings" })).getByRole("checkbox", { name: /The provider refuses the request/ }));
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect((await notifications().findAllByText("A route has at most 32 steps.")).length).toBeGreaterThan(0);
  });

  it("removes a step with the Delete key and from its menu", async () => {
    setUp();
    const { user } = await renderApp(projectPath("models/aliases/al_fast"));

    const haiku = await step(/Claude Haiku 4\.5/);
    await user.click(haiku);
    await user.keyboard("{Delete}");
    await waitFor(() => expect(screen.queryByRole("treeitem", { name: stepName(/Claude Haiku 4\.5/) })).not.toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Undo" }));
    await user.click(await step(/Claude Sonnet 4\.5/));
    await user.keyboard("{Shift>}{F10}{/Shift}");
    await user.click(await screen.findByRole("menuitem", { name: "Remove step" }));
    await waitFor(() => expect(screen.queryByRole("treeitem", { name: stepName(/Claude Sonnet 4\.5/) })).not.toBeInTheDocument());
    await user.click(await step(/Claude Haiku 4\.5/));
    await user.keyboard("{Delete}");
    expect(await step(/Claude Haiku 4\.5/)).toBeInTheDocument();
  });

  it("deletes the alias and goes back to the aliases", async () => {
    setUp();
    on("DELETE", ONE_ALIAS, () => undefined);
    const { user } = await renderApp(projectPath("models/aliases/al_fast"));

    await user.click(await screen.findByRole("button", { name: "Delete alias" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Delete the alias fast?" });
    await user.click(within(confirm).getByRole("button", { name: "Delete alias" }));
    expect(await notifications().findByText("Alias fast deleted")).toBeInTheDocument();
    await waitFor(() => expect(window.location.pathname).toBe(`/orgs/${ORG}/projects/${PROJECT}/models`));
  });

  it("stays on the page when leaving is called off", async () => {
    setUp();
    const { user } = await renderApp(projectPath("models/aliases/al_fast"));

    await user.click(await step(/Claude Haiku 4\.5/));
    await user.click(screen.getByRole("button", { name: "Remove step" }));
    await user.click(screen.getByRole("button", { name: "Back to aliases" }));
    const ask = await screen.findByRole("alertdialog", { name: "Leave without saving?" });
    await user.click(within(ask).getByRole("button", { name: "Cancel" }));
    expect(window.location.pathname).toBe(`/orgs/${ORG}/projects/${PROJECT}/models/aliases/al_fast`);
  });

  it("has no versions to show for an alias with none, and goes back to the editor", async () => {
    setUp();
    on("GET", AUDIT, ({ query }: { query: URLSearchParams }) =>
      page(
        query.get("target") === "al_fast"
          ? [
              { id: "evt_3", at: "2026-11-03T12:00:00Z", action: "alias.delete", actor: { kind: "service_account", id: "sa_deploy" }, after: null },
              { id: "evt_4", at: "2026-11-03T12:00:00Z", action: "alias.update", actor: { kind: "user", id: USER }, after: { name: 7 } },
            ]
          : [],
      ),
    );
    const { user } = await renderApp(projectPath("models/aliases/al_fast"));

    await user.click(await screen.findByRole("tab", { name: "Versions" }));
    expect(await screen.findByText("No versions yet")).toBeInTheDocument();
    await user.click(await step(/Claude Sonnet 4\.5/));
    expect(screen.queryByRole("heading", { name: "Model" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "Editor" }));
    expect(await screen.findByRole("heading", { name: "Model" })).toBeInTheDocument();
  });

  it("saves nothing from the keyboard when nothing changed, and warns before the tab closes with changes", async () => {
    setUp();
    const { user } = await renderApp(projectPath("models/aliases/al_fast"));

    await step(/Claude Haiku 4\.5/);
    await user.keyboard("{Control>}s{/Control}");
    expect(requests("PATCH", ONE_ALIAS)).toHaveLength(0);
    const quiet = new Event("beforeunload", { cancelable: true });
    window.dispatchEvent(quiet);
    expect(quiet.defaultPrevented).toBe(false);
    await user.click(await step(/Claude Haiku 4\.5/));
    await user.click(screen.getByRole("button", { name: "Remove step" }));
    const leaving = new Event("beforeunload", { cancelable: true });
    window.dispatchEvent(leaving);
    expect(leaving.defaultPrevented).toBe(true);
  });

  it("tests a request: the trace lights the path it takes and names each try", async () => {
    setUp({ one: planned() });
    projectData({ aliases: [planned()], models: models(), keys: [apiKey()] });
    on("POST", TRACE, ({ body }: { body: Traced }) =>
      body.request.headers.some((h) => h.value === "enterprise")
        ? {
            steps: [
              { type: "condition", path: "route", holds: true, rules: [true] },
              { type: "model", path: "route.then", catalogue_model_id: "cm_sonnet", attempt: 1 },
              { type: "model", path: "route.then.fallback", catalogue_model_id: "cm_haiku", attempt: 2 },
            ],
          }
        : {
            steps: [
              { type: "condition", path: "route", holds: false, rules: [false] },
              { type: "split", path: "route.else", branch: 1, weight: 50, draw: 70 },
              { type: "model", path: "route.else.branches.1.step", catalogue_model_id: "cm_haiku", attempt: 1 },
            ],
          },
    );
    const { user } = await renderApp(projectPath("models/aliases/al_fast"));

    await user.click(await screen.findByRole("tab", { name: "Test" }));
    const panel = await screen.findByRole("complementary", { name: "Step settings" });
    expect(within(panel).getByRole("heading", { name: "Test a request" })).toBeInTheDocument();
    expect(await within(panel).findByText("Doesn't hold; takes Else")).toBeInTheDocument();
    expect(within(panel).getByText("Drew 70 of 100; takes branch 2, 50%")).toBeInTheDocument();
    expect(within(panel).getByText("Try 1; tried first")).toBeInTheDocument();
    expect(within(panel).getByRole("textbox", { name: "Name of header 1" })).toHaveValue("x-plan");

    await user.type(within(panel).getByRole("textbox", { name: "Value of header 1" }), "enterprise");
    expect(await within(panel).findByText("Holds; takes Then")).toBeInTheDocument();
    expect(within(panel).getByText("Try 2; tried when try 1 fails")).toBeInTheDocument();
    const traced = screen.getAllByRole("treeitem").filter((i) => i.getAttribute("data-trace") === "on");
    expect(traced.map((i) => i.textContent)).toEqual([
      expect.stringMatching(/^Start/),
      expect.stringMatching(/^Condition/),
      expect.stringMatching(/^ModelTry 1Claude Sonnet 4\.5/),
      expect.stringMatching(/^ModelTry 2Claude Haiku 4\.5/),
    ]);
    const last = requests("POST", TRACE).at(-1)?.body as Traced;
    expect(last).toMatchObject({
      alias: "al_fast",
      request: { headers: [{ name: "x-plan", value: "enterprise" }], format: "openai_chat", input_tokens: 1000, key: null },
    });

    await user.click(within(panel).getByRole("combobox", { name: "API key" }));
    await user.click(await screen.findByRole("option", { name: /checkout-web/ }));
    await waitFor(() => expect((requests("POST", TRACE).at(-1)?.body as Traced).request).toMatchObject({ key: "key_web", member: USER }));
    const seed = (requests("POST", TRACE).at(-1)?.body as Traced).request.seed;
    await user.click(within(panel).getByRole("button", { name: "Draw again" }));
    await waitFor(() => expect((requests("POST", TRACE).at(-1)?.body as Traced).request.seed).not.toBe(seed));

    await user.click(within(panel).getByRole("button", { name: "Add header" }));
    await user.type(within(panel).getByRole("textbox", { name: "Name of header 2" }), "Cookie");
    expect(await within(panel).findByText("Routes can't read headers that carry credentials.")).toBeInTheDocument();
    await user.click(within(panel).getByRole("button", { name: "Remove header 2" }));
    expect(within(panel).queryByRole("textbox", { name: "Name of header 2" })).not.toBeInTheDocument();

    await user.click((await screen.findAllByRole("treeitem", { name: stepName(/Claude Haiku 4\.5/) }))[0] as HTMLElement);
    expect(await screen.findByRole("heading", { name: "Model" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Editor" })).toHaveAttribute("aria-selected", "true");
  });

  it("won't test a route that needs fixes, and says why a trace was refused", async () => {
    setUp();
    on("POST", TRACE, apiError(400, "invalid_request", "Send up to 20 headers.", { param: "request.headers" }));
    const { user } = await renderApp(projectPath("models/aliases/al_fast"));

    await user.click(await screen.findByRole("tab", { name: "Test" }));
    expect(await screen.findByText("Send up to 20 headers.")).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "Editor" }));
    await user.click(await screen.findByRole("treeitem", { name: "Add a fallback to Claude Haiku 4.5" }));
    await user.click(await screen.findByRole("menuitem", { name: "Fall back to a model" }));
    await user.click(screen.getByRole("tab", { name: "Test" }));
    expect(await screen.findByText("Fix the steps marked on the route first.")).toBeInTheDocument();
  });

  it("moves a fallback earlier or later and swaps a condition's branches from a step's actions", async () => {
    setUp({ one: planned() });
    on("PATCH", ONE_ALIAS, ({ body }: { body: { route: unknown } }) => ({ ...planned(), route: body.route }));
    const { user } = await renderApp(projectPath("models/aliases/al_fast"));

    const first = (await screen.findAllByRole("treeitem", { name: stepName(/Claude Sonnet 4\.5/) }))[0];
    await user.click(first as HTMLElement);
    await user.keyboard("{Shift>}{F10}{/Shift}");
    expect(screen.queryByRole("menuitem", { name: "Try earlier" })).not.toBeInTheDocument();
    await user.click(await screen.findByRole("menuitem", { name: "Try later" }));

    await user.click(screen.getAllByRole("treeitem", { name: stepName(/x-plan is enterprise/) })[0] as HTMLElement);
    await user.keyboard("{Shift>}{F10}{/Shift}");
    await user.click(await screen.findByRole("menuitem", { name: "Swap Then and Else" }));
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(await notifications().findByText("Alias fast saved")).toBeInTheDocument();
    const [body] = requests("PATCH", ONE_ALIAS).map(
      (r) => r.body as { route: { then: { type: string }; else: { model: string; fallback: { model: string } } } },
    );
    expect(body?.route.then.type).toBe("split");
    expect(body?.route.else.model).toBe("cm_haiku");
    expect(body?.route.else.fallback.model).toBe("cm_sonnet");
  });

  it("starts a new alias from a template and lets go of it once the route changes", async () => {
    signedIn();
    projectData({ aliases: [], models: models() });
    modelsApi({ models: models() });
    const { user } = await renderApp(projectPath("models/aliases/new"));

    const chain = await screen.findByRole("radio", { name: /A fallback chain/ });
    await user.click(chain);
    expect(chain).toBeChecked();
    expect(await step(/Claude Sonnet 4\.5/)).toBeInTheDocument();
    expect(await step(/Claude Haiku 4\.5/)).toBeInTheDocument();
    expect(await step(/No model yet/)).toBeInTheDocument();

    await user.click(screen.getByRole("radio", { name: /An A\/B test/ }));
    expect(await step(/By caller/)).toBeInTheDocument();
    await user.click(screen.getByRole("radio", { name: /By a header/ }));
    expect(await step(/x-plan is enterprise/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Undo" }));
    expect(await step(/By caller/)).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: /An A\/B test/ })).not.toBeChecked();
  });

  it("splits by a header and saves which header it reads", async () => {
    const split = () =>
      alias({
        route: {
          type: "split",
          by: "request",
          branches: [
            { weight: 50, step: modelRoute("cm_sonnet") },
            { weight: 50, step: modelRoute("cm_haiku") },
          ],
        },
      });
    setUp({ one: split() });
    on("PATCH", ONE_ALIAS, ({ body }: { body: { route: unknown } }) => ({ ...split(), route: body.route }));
    const { user } = await renderApp(projectPath("models/aliases/al_fast"));

    await user.click(await step(/Per request/));
    const panel = screen.getByRole("complementary", { name: "Step settings" });
    await user.click(within(panel).getByRole("radio", { name: "By header" }));
    expect(await step(/By header/)).toHaveTextContent("Name the header");
    const header = within(panel).getByRole("textbox", { name: /^Header/ });
    await user.type(header, "Authorization");
    expect(await within(panel).findByText("Routes can't read headers that carry credentials.")).toBeInTheDocument();
    await user.clear(header);
    await user.type(header, "X-User-Id");
    expect(header).toHaveValue("x-user-id");
    expect(await step(/By x-user-id/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(await notifications().findByText("Alias fast saved")).toBeInTheDocument();
    const [body] = requests("PATCH", ONE_ALIAS).map((r) => r.body as { route: { by: string; header: string } });
    expect(body?.route).toMatchObject({ by: "header", header: "x-user-id" });
  });
});
