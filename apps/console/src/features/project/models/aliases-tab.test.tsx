import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ORG, OTHER_PROJECT, PROJECT, signedIn, without } from "~/test/fixtures";
import { hoverReason, notifications } from "~/test/fixtures/org";
import type { AliasView } from "~/api/generated/types";
import { alias, catalogueModel, modelRoute, modelsApi, projectData, projectPath } from "~/test/fixtures/project";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { queryClient } from "~/query";

const ALIASES = "orgs/:org/projects/:project/aliases";
const PLAYGROUND_MODELS = ["orgs", ORG, "projects", PROJECT, "playground", "models"];
const OTHER_PLAYGROUND_MODELS = ["orgs", ORG, "projects", OTHER_PROJECT, "playground", "models"];
const playgroundReloads = (key: readonly unknown[]) => queryClient.getQueryState(key)?.isInvalidated;
const ONE_ALIAS = "orgs/:org/projects/:project/aliases/:alias";

const models = () => [
  catalogueModel(),
  catalogueModel({ id: "cm_haiku", model_id: "claude-haiku-4-5", effective: { ...catalogueModel().effective, display_name: "claude-haiku-4-5" } }),
];

const aliases = () => [
  alias(),
  alias({
    id: "al_haiku",
    name: "claude-haiku-4-5",
    route: modelRoute("cm_haiku"),
    models: [{ catalogue_model_id: "cm_haiku", target: "anthropic/claude-haiku-4-5" }],
    shadows_model: true,
  }),
];

function setUp({ rights, list = aliases() }: { rights?: string[]; list?: ReturnType<typeof aliases> } = {}) {
  signedIn(rights ? { projectRights: rights } : {});
  projectData({ aliases: list });
  modelsApi({ models: models() });
}

describe("AliasesTab", () => {
  it("lists each alias with its route and warns when it shadows a model", async () => {
    setUp();
    const { user } = await renderApp(projectPath("models?tab=aliases"));

    const fast = await screen.findByRole("gridcell", { name: /^fast/ });
    expect(fast).toHaveTextContent("Claude Sonnet 4.5");
    const haiku = screen.getByRole("gridcell", { name: /^claude-haiku-4-5/ });
    await hoverReason(user, within(haiku).getByText("Shadows a model"), "Apps that send `claude-haiku-4-5` get this alias, not the model with the same name.");
  });

  it("sums up a fallback chain, a branching route and a removed model", async () => {
    setUp({
      list: [
        alias({
          id: "al_sturdy",
          name: "sturdy",
          route: modelRoute("cm_sonnet", modelRoute("cm_haiku")),
          models: [
            { catalogue_model_id: "cm_sonnet", target: "anthropic/claude-sonnet-4-5" },
            { catalogue_model_id: "cm_haiku", target: "anthropic/claude-haiku-4-5" },
          ],
        }),
        alias({
          id: "al_tiered",
          name: "tiered",
          route: {
            type: "condition",
            match: "all",
            rules: [{ field: "header", name: "x-plan", op: "equals", value: "paid" }],
            then: modelRoute("cm_sonnet"),
            else: modelRoute("cm_gone"),
          },
          models: [
            { catalogue_model_id: "cm_sonnet", target: "anthropic/claude-sonnet-4-5" },
            { catalogue_model_id: "cm_gone", target: null },
          ],
        }),
      ],
    });
    await renderApp(projectPath("models?tab=aliases"));

    expect(await screen.findByRole("gridcell", { name: /^sturdy/ })).toHaveTextContent("Claude Sonnet 4.5 → claude-haiku-4-5");
    const tiered = screen.getByRole("gridcell", { name: /^tiered/ });
    expect(tiered).toHaveTextContent("Routes to 2 models by conditions");
    expect(tiered).toHaveTextContent("Model removed");
  });

  it("sums up splits alone and with conditions", async () => {
    const split: AliasView["route"] = {
      type: "split",
      by: "request",
      branches: [
        { weight: 50, step: modelRoute("cm_sonnet") },
        { weight: 50, step: modelRoute("cm_gone", modelRoute("cm_haiku")) },
      ],
    };
    const models3 = [
      { catalogue_model_id: "cm_sonnet", target: "anthropic/claude-sonnet-4-5" },
      { catalogue_model_id: "cm_gone", target: null },
      { catalogue_model_id: "cm_haiku", target: "anthropic/claude-haiku-4-5" },
    ];
    setUp({
      list: [
        alias({ id: "al_ab", name: "ab", route: split, models: models3 }),
        alias({
          id: "al_mixed",
          name: "mixed",
          route: { type: "condition", match: "any", rules: [{ field: "feature", op: "used", value: "images" }], then: split, else: modelRoute("cm_sonnet") },
          models: models3,
        }),
        alias({ id: "al_chain", name: "chain", route: modelRoute("cm_gone", modelRoute("cm_sonnet")), models: models3.slice(0, 2) }),
      ],
    });
    await renderApp(projectPath("models?tab=aliases"));

    expect(await screen.findByRole("gridcell", { name: /^ab/ })).toHaveTextContent("Splits requests across 3 models");
    expect(screen.getByRole("gridcell", { name: /^mixed/ })).toHaveTextContent("Routes to 3 models by conditions and splits");
    expect(screen.getByRole("gridcell", { name: /^chain/ })).toHaveTextContent("Removed model → Claude Sonnet 4.5");
  });

  it("creates an alias with the page's shortcut", async () => {
    setUp();
    const { user } = await renderApp(projectPath("models?tab=aliases"));
    await screen.findByRole("gridcell", { name: /^fast/ });
    await user.keyboard("c");
    expect(await screen.findByRole("heading", { name: "New alias" })).toBeInTheDocument();
  });

  it("offers nothing to create without Manage models", async () => {
    setUp({ rights: without("models.manage"), list: [] });
    await renderApp(projectPath("models?tab=aliases"));
    expect(await screen.findByText("No aliases yet")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "Create alias" })).toHaveLength(1);
  });

  it("says the aliases couldn't load instead of offering the first one", async () => {
    setUp();
    on("GET", ALIASES, apiError(503, "unavailable", "Kaeso can't reach its database right now; try again shortly."));
    await renderApp(projectPath("models?tab=aliases"));

    expect(await screen.findByText("The aliases couldn't load")).toBeInTheDocument();
    expect(screen.queryByText("No aliases yet")).not.toBeInTheDocument();
  });

  it("offers to create the first alias on its own page", async () => {
    setUp({ list: [] });
    const { user } = await renderApp(projectPath("models?tab=aliases"));

    expect(await screen.findByText("No aliases yet")).toBeInTheDocument();
    expect(screen.getByText("fast", { selector: "code" }).parentElement).toHaveTextContent(
      /^A name like fast that apps send as model; it falls back when a model fails\.$/,
    );
    const create = screen.getAllByRole("button", { name: "Create alias" });
    expect(create).toHaveLength(2);
    await user.click(create[1] as HTMLElement);
    expect(await screen.findByRole("heading", { name: "New alias" })).toBeInTheDocument();
    expect(window.location.pathname).toBe(`/orgs/${ORG}/projects/${PROJECT}/models/aliases/new`);
  });

  it("opens an alias's route from its row and from its menu", async () => {
    setUp();
    on("GET", ONE_ALIAS, alias());
    const { user } = await renderApp(projectPath("models?tab=aliases"));

    await user.click(await screen.findByRole("gridcell", { name: /^fast/ }));
    expect(await screen.findByRole("heading", { name: "fast" })).toBeInTheDocument();
    expect(window.location.pathname).toBe(`/orgs/${ORG}/projects/${PROJECT}/models/aliases/al_fast`);
    await user.click(screen.getByRole("button", { name: "Back to aliases" }));
    await user.click(await screen.findByRole("button", { name: "Actions for fast" }));
    await user.click(await screen.findByRole("menuitem", { name: "Edit route" }));
    expect(await screen.findByRole("heading", { name: "fast" })).toBeInTheDocument();
  });

  it("deletes an alias after confirming", async () => {
    setUp();
    queryClient.setQueryData(PLAYGROUND_MODELS, []);
    queryClient.setQueryData(OTHER_PLAYGROUND_MODELS, []);
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
    expect(playgroundReloads(PLAYGROUND_MODELS)).toBe(true);
    expect(playgroundReloads(OTHER_PLAYGROUND_MODELS)).toBe(false);
  });

  it("shows aliases read-only without Manage models", async () => {
    setUp({ rights: without("models.manage") });
    const { user } = await renderApp(projectPath("models?tab=aliases"));

    await screen.findByRole("gridcell", { name: /^fast/ });
    expect(screen.queryByRole("button", { name: "Actions for fast" })).not.toBeInTheDocument();
    const create = screen.getByRole("button", { name: "Create alias" });
    expect(create).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, create, /You need Manage models/);
  });
});
