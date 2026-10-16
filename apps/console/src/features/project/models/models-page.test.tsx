import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ALL_PROJECT_RIGHTS, signedIn } from "~/test/fixtures";
import { hoverReason } from "~/test/fixtures/org";
import { alias, catalogueModel, modelsApi, projectData, projectPath } from "~/test/fixtures/project";
import { renderApp } from "~/test/render";

const without = (...rights: string[]) => ALL_PROJECT_RIGHTS.filter((r) => !rights.includes(r));
const cells = () => screen.getAllByRole("gridcell").map((c) => c.textContent);

const catalogue = () => [
  catalogueModel({ aliases: ["fast", "default", "cheap"], overrides: { input_price: "2.5", output_price: null } }),
  catalogueModel({
    id: "cm_haiku",
    model_id: "claude-haiku-4-5",
    listed: false,
    delisted_at: "2026-09-01T00:00:00Z",
    effective: {
      display_name: "claude-haiku-4-5",
      input_modalities: ["text", "pdf"],
      output_modalities: ["text"],
      context_limit: 1_500_000,
      output_limit: 8000,
      input_price: "1",
      output_price: "5",
      cache_read_price: "0.1",
      cache_write_price: "1.25",
      tool_call: false,
      reasoning: false,
      release_date: "2025-10-15",
    },
  }),
  catalogueModel({
    id: "cm_embed",
    provider: "openai",
    model_id: "text-embedding-3",
    egress_dialect: "openai_embeddings",
    custom: true,
    effective: {
      display_name: null,
      input_modalities: ["text"],
      output_modalities: ["embedding"],
      context_limit: null,
      input_price: "0.13",
      output_price: "0",
      cache_read_price: "0.01",
    },
  }),
  catalogueModel({
    id: "cm_gpt",
    provider: "openai",
    model_id: "gpt-5",
    egress_dialect: "openai_responses",
    effective: { display_name: "GPT-5", structured_output: true, input_price: null, output_price: null, cache_write_price: "2", release_date: "2025-08-07" },
  }),
];

function setUp(rights?: string[]) {
  signedIn(rights ? { projectRights: rights } : {});
  projectData({ aliases: [alias()] });
  modelsApi({ models: catalogue() });
}

const rowsOf = (group: string) => {
  const all = cells();
  const start = all.findIndex((c) => c?.startsWith(group));
  const rest = all.slice(start + 1);
  const end = rest.findIndex((c) => /^(Anthropic|OpenAI)\d+$/.test(c ?? ""));
  return (end < 0 ? rest : rest.slice(0, end)).map((c) => c?.split(/No longer|Overridden|Custom|messages|responses|embeddings/)[0]);
};

describe("ModelsPage", () => {
  it("groups the catalogue by provider with each model's flags, aliases, format, context and price", async () => {
    setUp();
    await renderApp(projectPath("models"));

    expect(await screen.findByText("gpt-5")).toBeInTheDocument();
    expect(cells()).toEqual([
      "Anthropic2",
      "claude-haiku-4-5No longer listedmessages1.5M$1 / $5Oct 2025",
      "Claude Sonnet 4.5claude-sonnet-4-5Overridden 1fastdefault+1messages200k$3 / $15Sept 2025",
      "OpenAI2",
      "GPT-5gpt-5responses——Aug 2025",
      "text-embedding-3Customembeddings—$0.13",
    ]);
    expect(screen.getByRole("tab", { name: /Catalogue/ })).toHaveTextContent("4");
    expect(await screen.findByRole("tab", { name: /Aliases/ })).toHaveTextContent("1");
  });

  it("shows a missing price as a dash and a format it doesn't know by its id", async () => {
    signedIn();
    projectData();
    const base = catalogueModel().effective;
    modelsApi({
      models: [
        catalogueModel({ effective: { ...base, display_name: null, input_price: null, output_price: "5", release_date: null } }),
        catalogueModel({
          id: "cm_opus",
          model_id: "claude-opus-4-1",
          egress_dialect: "bedrock_converse",
          effective: { ...base, display_name: null, release_date: null },
        }),
      ],
    });
    await renderApp(projectPath("models"));

    await screen.findByText("claude-opus-4-1");
    expect(cells()).toEqual(["Anthropic2", "claude-opus-4-1bedrock_converse200k$3 / $15", "claude-sonnet-4-5messages200k— / $5"]);
  });

  it("explains each price with its cache prices", async () => {
    setUp();
    const { user } = await renderApp(projectPath("models"));

    await hoverReason(user, await screen.findByText("$3 / $15"), "Per million input / output tokens");
    await hoverReason(user, screen.getByText("$1 / $5"), /^Per million input \/ output tokens; cache read \$0\.10, cache write \$1\.25$/);
    await hoverReason(user, screen.getByText("$0.13"), /^Per million input tokens; cache read \$0\.01$/);
    const gpt = screen.getByRole("gridcell", { name: /gpt-5/ });
    await hoverReason(user, within(gpt).getAllByText("—")[1] as HTMLElement, /cache write \$2\.00$/);
  });

  it("sorts each provider's models by price or newest release", async () => {
    setUp();
    const { user, router } = await renderApp(projectPath("models?sort=price"));

    await screen.findByText("gpt-5");
    expect(rowsOf("OpenAI")).toEqual(["text-embedding-3", "GPT-5gpt-5"]);
    await user.click(screen.getByRole("combobox", { name: "Sort" }));
    await user.click(await screen.findByRole("option", { name: "Newest release" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ sort: "release" }));
    expect(rowsOf("OpenAI")).toEqual(["GPT-5gpt-5", "text-embedding-3"]);
    expect(rowsOf("Anthropic")).toEqual(["claude-haiku-4-5", "Claude Sonnet 4.5claude-sonnet-4-5"]);
    await user.click(screen.getByRole("combobox", { name: "Sort" }));
    await user.click(await screen.findByRole("option", { name: "Model id" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("orders models with the same price or release by their id, and models without a release last", async () => {
    signedIn();
    projectData();
    const model = (id: string, release: string | null) =>
      catalogueModel({ id: `cm_${id}`, model_id: id, effective: { ...catalogueModel().effective, display_name: null, release_date: release } });
    modelsApi({ models: [model("claude-sonnet-4-5", "2025-09-29"), model("claude-haiku-4-5", null), model("claude-opus-4-1", "2025-09-29")] });
    const { user } = await renderApp(projectPath("models?sort=price"));

    await screen.findByText("claude-opus-4-1");
    expect(rowsOf("Anthropic")).toEqual(["claude-haiku-4-5", "claude-opus-4-1", "claude-sonnet-4-5"]);
    await user.click(screen.getByRole("combobox", { name: "Sort" }));
    await user.click(await screen.findByRole("option", { name: "Newest release" }));
    await waitFor(() => expect(rowsOf("Anthropic")).toEqual(["claude-opus-4-1", "claude-sonnet-4-5", "claude-haiku-4-5"]));
  });

  it("filters by provider, capability and overrides in the URL", async () => {
    setUp();
    const { user, router } = await renderApp(projectPath("models"));

    await screen.findByText("gpt-5");
    await user.click(screen.getByRole("combobox", { name: "Provider" }));
    await user.click(await screen.findByRole("option", { name: "OpenAI" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ provider: "openai" }));
    expect(screen.queryByText("claude-haiku-4-5")).not.toBeInTheDocument();
    await user.click(screen.getByRole("combobox", { name: "Provider" }));
    await user.click(await screen.findByRole("option", { name: "All providers" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));

    await user.click(screen.getByRole("combobox", { name: "Capability" }));
    await user.click(await screen.findByRole("option", { name: "PDF input" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ cap: "pdf" }));
    expect(rowsOf("Anthropic")).toEqual(["claude-haiku-4-5"]);
    await user.click(screen.getByRole("combobox", { name: "Capability" }));
    await user.click(await screen.findByRole("option", { name: "Any capability" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));

    await user.click(screen.getByRole("switch", { name: "Only overridden" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ overridden: true }));
    expect(cells()).toHaveLength(2);
    expect(screen.getByText("claude-sonnet-4-5")).toBeInTheDocument();
    await user.click(screen.getByRole("switch", { name: "Only overridden" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("says when no model matches the filters and clears them", async () => {
    setUp();
    const { user, router } = await renderApp(projectPath("models?provider=openai&cap=vision&overridden=true"));

    expect(await screen.findByText("No models match these filters")).toBeInTheDocument();
    expect(screen.getByText("Clear the filters to see the whole catalogue.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Clear filters" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
    expect(await screen.findByText("gpt-5")).toBeInTheDocument();
  });

  it("offers to add the first models to an empty catalogue", async () => {
    signedIn();
    projectData();
    modelsApi();
    const { user } = await renderApp(projectPath("models"));

    expect(await screen.findByText("No models yet")).toBeInTheDocument();
    expect(screen.getByText("Requests may only name catalogue models; add them from models.dev with their prices.")).toBeInTheDocument();
    const add = screen.getAllByRole("button", { name: "Add models" });
    expect(add).toHaveLength(2);
    await user.click(add[1] as HTMLElement);
    expect(await screen.findByRole("dialog", { name: /Add models/ })).toBeInTheDocument();
  });

  it("opens the add dialog from the header and from ?add=true", async () => {
    setUp();
    const { user } = await renderApp(projectPath("models?add=true"));

    const dialog = await screen.findByRole("dialog", { name: /Add models/ });
    await user.click(within(dialog).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: /Add models/ })).not.toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Add models" }));
    expect(await screen.findByRole("dialog", { name: /Add models/ })).toBeInTheDocument();
  });

  it("opens a model's sheet from its row and closes it", async () => {
    setUp();
    const { user, router } = await renderApp(projectPath("models"));

    await user.click(await screen.findByRole("gridcell", { name: /claude-sonnet-4-5/ }));
    await waitFor(() => expect(router.state.location.search).toEqual({ model: "cm_sonnet" }));
    const sheet = await screen.findByRole("dialog", { name: "Claude Sonnet 4.5" });
    await user.click(within(sheet).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("switches to the aliases tab and back", async () => {
    setUp();
    const { user, router } = await renderApp(projectPath("models?provider=openai"));

    await screen.findByText("gpt-5");
    await user.click(screen.getByRole("tab", { name: /Aliases/ }));
    await waitFor(() => expect(router.state.location.search).toEqual({ tab: "aliases", provider: "openai" }));
    expect(await screen.findByRole("button", { name: "Create alias" })).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: /Catalogue/ }));
    await waitFor(() => expect(router.state.location.search).toEqual({ provider: "openai" }));
  });

  it("explains why models can't be added without Manage models", async () => {
    signedIn({ projectRights: without("models.manage") });
    projectData();
    modelsApi();
    const { user } = await renderApp(projectPath("models"));

    expect(await screen.findByText("No models yet")).toBeInTheDocument();
    const add = screen.getAllByRole("button", { name: "Add models" });
    expect(add).toHaveLength(1);
    expect(add[0]).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, add[0] as HTMLElement, /You need Manage models/);
  });
});
