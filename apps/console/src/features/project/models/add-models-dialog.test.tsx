import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { signedIn } from "~/test/fixtures";
import { fill, hoverReason, notifications } from "~/test/fixtures/org";
import { catalogueModel, modelsApi, preview, projectData, projectPath, searchHit } from "~/test/fixtures/project";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const hits = () => [
  searchHit(),
  searchHit({ model_id: "gpt-5-mini", name: "GPT-5 mini", family: "gpt-mini", release_date: null }),
  searchHit({ model_id: "gpt-4o", name: "gpt-4o", family: null, catalogue_model_id: "cm_4o" }),
  searchHit({ model_id: "gpt-3.5-turbo", name: "GPT-3.5 Turbo", deprecated: true }),
  searchHit({ provider: "anthropic", model_id: "claude-opus-4-1", name: "Claude Opus 4.1", family: "claude-opus" }),
];

function setUp(previews = [preview()]) {
  signedIn();
  projectData();
  modelsApi({ models: [catalogueModel()], hits: hits(), previews });
}

const open = async () => {
  const view = await renderApp(projectPath("models?add=true"));
  const dialog = await screen.findByRole("dialog", { name: /Add models/ });
  return { ...view, dialog };
};

async function pickProvider(user: Awaited<ReturnType<typeof open>>["user"], dialog: HTMLElement, name: string) {
  await user.click(within(dialog).getByRole("button", { name: "Provider" }));
  await user.click(await screen.findByRole("option", { name: new RegExp(`^${name}`) }));
}

const BATCH = "orgs/:org/projects/:project/models/batch";
const MODELS = "orgs/:org/projects/:project/models";
const SEARCH = "orgs/:org/projects/:project/models/search";
const searches = () => requests("GET", SEARCH).map((r) => Object.fromEntries(r.query));

describe("AddModelsDialog", () => {
  it("adds the chosen models from models.dev and says which were already there", async () => {
    setUp();
    on("POST", BATCH, { added: [catalogueModel({ id: "cm_gpt5", model_id: "gpt-5" })], skipped: ["gpt-5-mini"] });
    const { user, dialog } = await open();

    expect(await within(dialog).findByText("gpt-5-mini")).toBeInTheDocument();
    expect(searches()).toContainEqual({ provider: "openai", include_deprecated: "false", limit: "50" });
    expect(within(dialog).getByRole("gridcell", { name: /gpt-4o/ })).toHaveTextContent("Already in catalogue");
    expect(within(dialog).queryByText("gpt-3.5-turbo")).not.toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Add models" })).toBeDisabled();
    await user.click(within(dialog).getByRole("checkbox", { name: "Select gpt-5" }));
    expect(within(dialog).getByRole("button", { name: "Review values" })).toBeInTheDocument();
    await user.click(within(dialog).getByRole("gridcell", { name: /GPT-5 mini/ }));
    expect(within(dialog).queryByRole("button", { name: "Review values" })).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Add 2 models" }));

    expect(await notifications().findByText("Added 1 model")).toBeInTheDocument();
    expect(notifications().getByText("1 already in the catalogue")).toBeInTheDocument();
    expect(requests("POST", BATCH).map((r) => r.body)).toEqual([{ provider: "openai", model_ids: ["gpt-5", "gpt-5-mini"] }]);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: /Add models/ })).not.toBeInTheDocument());
  });

  it("starts in the search box; Space selects rows and ⌘Enter adds them", async () => {
    setUp();
    on("POST", BATCH, { added: [catalogueModel({ id: "cm_gpt5", model_id: "gpt-5" })], skipped: [] });
    const { user, dialog } = await open();

    await within(dialog).findByText("gpt-5-mini");
    expect(within(dialog).getByRole("searchbox", { name: "Search models" })).toHaveFocus();
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(requests("POST", BATCH)).toEqual([]);
    await user.tab();
    await user.tab();
    expect(within(dialog).getByRole("gridcell", { name: /^Select gpt-5 / })).toHaveFocus();
    await user.keyboard(" ");
    expect(within(dialog).getByRole("button", { name: "Add 1 model" })).toBeInTheDocument();
    await user.keyboard("{ArrowDown} ");
    expect(within(dialog).getByRole("button", { name: "Add 2 models" })).toBeInTheDocument();
    await user.keyboard(" ");
    expect(within(dialog).getByRole("button", { name: "Add 1 model" })).toBeInTheDocument();
    await user.keyboard("{Meta>}{Enter}{/Meta}");

    expect(await notifications().findByText("Added 1 model")).toBeInTheDocument();
    expect(requests("POST", BATCH).map((r) => r.body)).toEqual([{ provider: "openai", model_ids: ["gpt-5"] }]);
  });

  it("opens a model's values with Enter or a click, and Back returns to the search", async () => {
    setUp();
    on("POST", MODELS, catalogueModel({ id: "cm_gpt5", model_id: "gpt-5" }));
    const { user, dialog } = await open();

    await within(dialog).findByText("gpt-5-mini");
    await user.tab();
    await user.tab();
    await user.keyboard("{Enter}");
    const reviewing = await screen.findByRole("dialog", { name: /Review gpt-5/ });
    expect(within(reviewing).getByRole("textbox", { name: "Display name" })).toHaveFocus();
    expect(requests("POST", BATCH)).toEqual([]);
    await user.click(screen.getByRole("button", { name: "Back" }));
    expect(await within(dialog).findByRole("button", { name: "Add models" })).toBeDisabled();
    expect(within(dialog).getByRole("gridcell", { name: /^Select gpt-5 / })).toHaveFocus();

    await user.click(within(dialog).getByRole("gridcell", { name: /^Select gpt-5 / }));
    const review = await screen.findByRole("dialog", { name: /Review gpt-5/ });
    expect(within(review).getByRole("textbox", { name: "Display name" })).toHaveFocus();
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(await notifications().findByText("Added gpt-5")).toBeInTheDocument();
    expect(requests("POST", MODELS).map((r) => r.body)).toEqual([{ provider: "openai", model_id: "gpt-5", custom: false, overrides: {} }]);
  });

  it("goes back to the search from the custom model and every-model steps", async () => {
    setUp();
    const { user, dialog } = await open();

    await user.click(await within(dialog).findByRole("button", { name: "Add a custom model" }));
    expect(await screen.findByRole("dialog", { name: /Add a custom model/ })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Back" }));
    await user.click(await screen.findByRole("button", { name: "Add every current OpenAI model" }));
    expect(await screen.findByText("Models that models.dev adds later aren't added here on their own.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Back" }));
    expect(await screen.findByRole("dialog", { name: /Add models/ })).toBeInTheDocument();
    expect(screen.getByRole("searchbox", { name: "Search models" })).toBeInTheDocument();
  });

  it("opens on an empty search with nothing chosen after a cancelled one", async () => {
    setUp();
    const { user, dialog } = await open();

    await within(dialog).findByText("gpt-5-mini");
    await user.click(within(dialog).getByRole("checkbox", { name: "Select gpt-5" }));
    await user.type(within(dialog).getByRole("searchbox", { name: "Search models" }), "mini");
    await user.click(within(dialog).getByRole("button", { name: "Add a custom model" }));
    const custom = await screen.findByRole("dialog", { name: /Add a custom model/ });
    await user.type(within(custom).getByRole("textbox", { name: "Model id" }), "acme-internal-1");
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    await user.click(screen.getAllByRole("button", { name: "Add models" })[0] as HTMLElement);
    const again = await screen.findByRole("dialog", { name: /Add models/ });
    expect(within(again).getByRole("searchbox", { name: "Search models" })).toHaveValue("");
    expect(await within(again).findByRole("checkbox", { name: "Select gpt-5" })).not.toBeChecked();
    expect(within(again).getByRole("button", { name: "Add models" })).toBeDisabled();
    expect(within(again).queryByRole("button", { name: "Back" })).not.toBeInTheDocument();
    expect(screen.queryByDisplayValue("acme-internal-1")).not.toBeInTheDocument();
  });

  it("searches by name, includes deprecated models and switches provider", async () => {
    setUp();
    const { user, dialog } = await open();

    await within(dialog).findByText("gpt-5-mini");
    await user.click(within(dialog).getByRole("switch"));
    expect(await within(dialog).findByText("Deprecated")).toBeInTheDocument();
    await user.type(within(dialog).getByRole("searchbox", { name: "Search models" }), "mini");
    await waitFor(() => expect(searches()).toContainEqual({ q: "mini", provider: "openai", include_deprecated: "true", limit: "50" }));
    await waitFor(() => expect(within(dialog).queryByText("gpt-4o")).not.toBeInTheDocument());
    await user.type(within(dialog).getByRole("searchbox", { name: "Search models" }), "zzz");
    expect(await within(dialog).findByText("No models found")).toBeInTheDocument();
    expect(within(dialog).getByText("Try another name, or include deprecated models.")).toBeInTheDocument();
    await user.clear(within(dialog).getByRole("searchbox", { name: "Search models" }));
    await pickProvider(user, dialog, "Anthropic");
    expect(await within(dialog).findByText("claude-opus-4-1")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Add every current Anthropic model" })).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Provider" })).toHaveTextContent("Anthropic");

    await user.click(within(dialog).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: /Add models/ })).not.toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Add models" }));
    const again = await screen.findByRole("dialog", { name: /Add models/ });
    expect(within(again).getByRole("button", { name: "Provider" })).toHaveTextContent("Anthropic");
  });

  it("says when models.dev lists no models for the provider", async () => {
    signedIn();
    projectData();
    modelsApi();
    const { dialog } = await open();

    expect(await within(dialog).findByText("No models found")).toBeInTheDocument();
    expect(within(dialog).getByText("models.dev lists no OpenAI models.")).toBeInTheDocument();
  });

  it("reviews one model's values and saves the changes as overrides", async () => {
    setUp();
    on("POST", MODELS, catalogueModel({ id: "cm_gpt5", model_id: "gpt-5" }));
    const { user, dialog } = await open();

    await user.click(await within(dialog).findByRole("checkbox", { name: "Select gpt-5" }));
    await user.click(within(dialog).getByRole("button", { name: "Review values" }));
    const review = await screen.findByRole("dialog", { name: /Review gpt-5/ });
    expect(within(review).getByRole("textbox", { name: "Display name" })).toHaveValue("GPT-5");
    expect(within(review).queryByRole("switch", { name: "Adaptive thinking" })).not.toBeInTheDocument();
    const output = within(review).getByRole("textbox", { name: "Output price" });
    await user.clear(output);
    await user.type(output, "12");
    await user.type(within(review).getByRole("textbox", { name: "Timeout" }), "1.5");
    await hoverReason(user, within(review).getByRole("button", { name: "Add model" }), "Limits and the timeout are whole numbers.");
    await user.click(within(review).getByRole("textbox", { name: "Timeout" }));
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(requests("POST", MODELS)).toEqual([]);
    await user.clear(within(review).getByRole("textbox", { name: "Timeout" }));
    await user.click(within(review).getByRole("switch", { name: "Tool calling" }));
    await user.click(within(review).getByRole("button", { name: "Add model" }));

    expect(await notifications().findByText("Added gpt-5")).toBeInTheDocument();
    expect(requests("POST", MODELS).map((r) => r.body)).toEqual([
      { provider: "openai", model_id: "gpt-5", custom: false, overrides: { output_price: "12", tool_call: false } },
    ]);
  });

  it("says when models.dev no longer lists the model under review", async () => {
    setUp([]);
    const { user, dialog } = await open();

    await user.click(await within(dialog).findByRole("checkbox", { name: "Select gpt-5" }));
    await user.click(within(dialog).getByRole("button", { name: "Review values" }));
    expect((await notifications().findAllByText("models.dev doesn't list gpt-5 any more.")).length).toBeGreaterThan(0);
    expect(within(dialog).getByRole("button", { name: "Add 1 model" })).toBeInTheDocument();
  });

  it("shows the API's message when the review can't load", async () => {
    setUp();
    on("GET", "orgs/:org/projects/:project/models/preview", apiError(400, "invalid_request", "That provider is unknown."));
    const { user, dialog } = await open();

    await user.click(await within(dialog).findByRole("checkbox", { name: "Select gpt-5" }));
    await user.click(within(dialog).getByRole("button", { name: "Review values" }));
    expect((await notifications().findAllByText("That provider is unknown.")).length).toBeGreaterThan(0);
  });

  it("adds a custom model models.dev doesn't list once its required values are in", async () => {
    setUp();
    on("POST", MODELS, catalogueModel({ id: "cm_mine", provider: "openai", model_id: "my-model", custom: true }));
    const { user, dialog } = await open();

    await user.click(await within(dialog).findByRole("button", { name: "Add a custom model" }));
    const custom = await screen.findByRole("dialog", { name: /Add a custom model/ });
    expect(within(custom).getByRole("textbox", { name: "Model id" })).toHaveFocus();
    const continueButton = within(custom).getByRole("button", { name: "Continue" });
    expect(continueButton).toBeDisabled();
    await fill(user, within(custom).getByRole("textbox", { name: "Model id" }), " my-model ");
    await user.click(within(custom).getByRole("button", { name: "Continue" }));
    expect(
      await within(custom).findByText("models.dev doesn't list it. Inputs, outputs, limits, input and output prices and the format are required."),
    ).toBeInTheDocument();

    const add = () => within(custom).getByRole("button", { name: "Add model" });
    await hoverReason(user, add(), "A custom model needs its inputs and outputs.");
    await user.click(within(custom).getByRole("textbox", { name: "Model id" }));
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(requests("POST", MODELS)).toEqual([]);
    await user.click(within(custom).getAllByRole("checkbox", { name: "Text" })[0] as HTMLElement);
    await user.click(within(custom).getAllByRole("checkbox", { name: "Text" })[1] as HTMLElement);
    await hoverReason(user, add(), "A custom model needs its context and output limits.");
    await fill(user, within(custom).getByRole("textbox", { name: "Context" }), "32000");
    await fill(user, within(custom).getByRole("textbox", { name: "Output limit" }), "4000");
    await hoverReason(user, add(), "A custom model needs its input and output prices.");
    await fill(user, within(custom).getByRole("textbox", { name: "Input price" }), "0.5");
    await fill(user, within(custom).getByRole("textbox", { name: "Output price" }), "1.5");
    await hoverReason(user, add(), "A custom model needs the format Kaeso sends it.");
    await user.click(within(custom).getByRole("combobox", { name: "Sends as" }));
    await user.click(await screen.findByRole("option", { name: "OpenAI Chat Completions" }));
    await user.click(within(custom).getByRole("combobox", { name: "Effort levels" }));
    await user.click(await screen.findByRole("option", { name: "low" }));
    await user.keyboard("{Escape}");
    await fill(user, within(custom).getByRole("textbox", { name: "Timeout" }), "60");
    await user.click(add());

    expect(await notifications().findByText("Added my-model")).toBeInTheDocument();
    expect(requests("POST", MODELS).map((r) => r.body)).toEqual([
      {
        provider: "openai",
        model_id: "my-model",
        custom: true,
        overrides: {
          input_modalities: ["text"],
          output_modalities: ["text"],
          context_limit: 32000,
          output_limit: 4000,
          input_price: "0.5",
          output_price: "1.5",
          egress_dialect: "openai_chat",
          tool_call: false,
          reasoning: false,
          structured_output: false,
          temperature: false,
          efforts: ["low"],
          timeout_seconds: 60,
        },
      },
    ]);
  });

  it("treats a custom model as unlisted when models.dev can't be asked", async () => {
    setUp();
    on("GET", "orgs/:org/projects/:project/models/preview", apiError(400, "invalid_request", "That provider is unknown."));
    const { user, dialog } = await open();

    await user.click(await within(dialog).findByRole("button", { name: "Add a custom model" }));
    const custom = await screen.findByRole("dialog", { name: /Add a custom model/ });
    await user.type(within(custom).getByRole("textbox", { name: "Model id" }), "my-model{Enter}");
    expect(
      await within(custom).findByText("models.dev doesn't list it. Inputs, outputs, limits, input and output prices and the format are required."),
    ).toBeInTheDocument();
  });

  it("fills a custom model's values in when models.dev lists it", async () => {
    setUp([
      preview({
        provider: "anthropic",
        model_id: "claude-opus-4-1",
        egress_dialect: "anthropic_messages",
        fields: { ...preview().fields, display_name: "Claude Opus 4.1" },
      }),
    ]);
    on("POST", MODELS, catalogueModel({ id: "cm_opus", model_id: "claude-opus-4-1" }));
    const { user, dialog } = await open();

    await pickProvider(user, dialog, "Anthropic");
    await user.click(await within(dialog).findByRole("button", { name: "Add a custom model" }));
    const custom = await screen.findByRole("dialog", { name: /Add a custom model/ });
    expect(within(custom).getByText("The id Anthropic uses, sent as `model`.")).toBeInTheDocument();
    await user.type(within(custom).getByRole("textbox", { name: "Model id" }), "claude-opus-4-1{Enter}");
    expect(await within(custom).findByText("models.dev lists claude-opus-4-1: its values are filled in.")).toBeInTheDocument();
    expect(within(custom).getByRole("textbox", { name: "Display name" })).toHaveValue("Claude Opus 4.1");
    await user.click(within(custom).getByRole("switch", { name: "Adaptive thinking" }));
    await user.click(within(custom).getByRole("button", { name: "Add model" }));

    expect(await notifications().findByText("Added claude-opus-4-1")).toBeInTheDocument();
    expect(requests("POST", MODELS).map((r) => r.body)).toEqual([
      { provider: "anthropic", model_id: "claude-opus-4-1", custom: false, overrides: { adaptive_thinking: true } },
    ]);
  });

  it("adds every current model of the provider", async () => {
    setUp();
    on("POST", BATCH, { added: [catalogueModel()], skipped: [] });
    const { user, dialog } = await open();

    await user.click(await within(dialog).findByRole("button", { name: "Add every current OpenAI model" }));
    const all = await screen.findByRole("dialog", { name: /Add every current OpenAI model/ });
    expect(within(all).getByText("Adds each current OpenAI text or embedding model from models.dev and skips ones already here.")).toBeInTheDocument();
    expect(within(all).getByText("Models that models.dev adds later aren't added here on their own.")).toBeInTheDocument();
    await user.click(within(all).getByRole("button", { name: "Add every current OpenAI model" }));

    expect(await notifications().findByText("Added 1 model")).toBeInTheDocument();
    expect(notifications().queryByText(/already in the catalogue/)).not.toBeInTheDocument();
    expect(requests("POST", BATCH).map((r) => r.body)).toEqual([{ provider: "openai", all_current: true }]);
  });

  it("keeps the dialog open with the API's message when adding fails", async () => {
    setUp();
    on("POST", BATCH, apiError(403, "permission", "You can't add models here."));
    const { user, dialog } = await open();

    await user.click(await within(dialog).findByRole("checkbox", { name: "Select gpt-5" }));
    await user.click(within(dialog).getByRole("button", { name: "Add 1 model" }));
    expect((await notifications().findAllByText("You can't add models here.")).length).toBeGreaterThan(0);
    expect(screen.getByRole("dialog", { name: /Add models/ })).toBeInTheDocument();
  });
});
