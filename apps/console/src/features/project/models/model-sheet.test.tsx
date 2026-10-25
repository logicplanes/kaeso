import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { signedIn, without } from "~/test/fixtures";
import { hoverReason, notifications } from "~/test/fixtures/org";
import { catalogueModel, modelsApi, projectData, projectPath } from "~/test/fixtures/project";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const ONE_MODEL = "orgs/:org/projects/:project/models/:model";

const sonnet = () =>
  catalogueModel({
    aliases: ["fast", "default"],
    overrides: { display_name: "Sonnet", input_price: "2.5" },
    effective: {
      ...catalogueModel().effective,
      display_name: "Sonnet",
      input_price: "2.5",
      cache_read_price: "0.3",
      efforts: ["low", "high"],
      timeout_seconds: 120,
      adaptive_thinking: true,
      egress_dialect: null,
    },
  });

function setUp(models = [sonnet()], rights?: string[]) {
  signedIn(rights ? { projectRights: rights } : {});
  projectData();
  modelsApi({ models });
}

const sheet = (name: string | RegExp) => screen.findByRole("dialog", { name });

describe("ModelSheet", () => {
  it("shows the model's details and where each value in use comes from", async () => {
    setUp();
    await renderApp(projectPath("models?model=cm_sonnet"));

    const s = await sheet("Sonnet");
    await within(s).findByText("Values in use");
    expect(within(s).getByText("Overridden 2")).toBeInTheDocument();
    expect(s).toHaveTextContent("Aliasesfast, default");
    expect(s).toHaveTextContent("Added15 Oct 2026 by Ada Okafor (you)");
    expect(s).toHaveTextContent("Display nameSonnetoverrideReset to models.dev");
    expect(s).toHaveTextContent("Input price$2.50 / M tokensoverrideReset to models.dev");
    expect(s).toHaveTextContent("Cache write price—Sends as");
    expect(s).toHaveTextContent("Sends asAnthropic MessagesKaeso");
    expect(s).toHaveTextContent("Structured outputNomodels.dev");
    expect(s).toHaveTextContent("Effortslow, highmodels.dev");
    expect(s).toHaveTextContent("Timeout120 smodels.dev");
    expect(s).toHaveTextContent("Released29 Sep 2025models.dev");
  });

  it("marks a model models.dev no longer lists", async () => {
    setUp([catalogueModel({ listed: false, delisted_at: "2026-09-01T00:00:00Z" }), catalogueModel({ id: "cm_old", model_id: "claude-2", listed: false })]);
    const { router } = await renderApp(projectPath("models?model=cm_sonnet"));

    const s = await sheet("Claude Sonnet 4.5");
    expect(await within(s).findByText("No longer listed since 1 Sep 2026")).toBeInTheDocument();
    expect(within(s).getByText("models.dev stopped listing it; it keeps its last known values.")).toBeInTheDocument();
    expect(within(s).queryByText(/^Overridden/)).not.toBeInTheDocument();
    await router.navigate({ to: projectPath("models"), search: { model: "cm_old" } });
    expect(await within(await sheet("Claude Sonnet 4.5")).findByText("No longer listed")).toBeInTheDocument();
  });

  it("says whether the provider serves the model and whether it is deprecated", async () => {
    setUp([
      catalogueModel({ availability: "unavailable", availability_checked_at: "2026-10-16T08:00:00Z", deprecated: true }),
      catalogueModel({ id: "cm_ok", model_id: "claude-haiku-4-5", availability: "available", availability_checked_at: "2026-10-16T08:00:00Z" }),
      catalogueModel({ id: "cm_new", model_id: "claude-opus-4-1" }),
      catalogueModel({ id: "cm_quiet", model_id: "claude-3-opus", availability: "unavailable" }),
      catalogueModel({ id: "cm_seen", model_id: "claude-3-5-haiku", availability: "available" }),
    ]);
    const { router } = await renderApp(projectPath("models?model=cm_sonnet"));

    const s = await sheet("Claude Sonnet 4.5");
    expect(await within(s).findByText("Unavailable")).toBeInTheDocument();
    expect(within(s).getByText("Deprecated")).toBeInTheDocument();
    expect(within(s).getByText("Anthropic doesn't serve it to this project's provider key; check the key's access or remove the model.")).toBeInTheDocument();
    expect(within(s).getByText("models.dev marks it deprecated; plan a move to a newer model.")).toBeInTheDocument();
    expect(s).toHaveTextContent("AvailabilityUnavailable since 16 Oct 2026");
    for (const [id, text] of [
      ["cm_ok", "AvailabilityAvailable, checked 16 Oct 2026"],
      ["cm_new", "AvailabilityNot checked"],
      ["cm_quiet", "AvailabilityUnavailableAdded"],
      ["cm_seen", "AvailabilityAvailableAdded"],
    ] as const) {
      await router.navigate({ to: projectPath("models"), search: { model: id } });
      await waitFor(() => expect(screen.getByRole("dialog")).toHaveTextContent(text));
    }
    expect(within(screen.getByRole("dialog")).queryByText("Deprecated")).not.toBeInTheDocument();
  });

  it("shows a custom model's values as set here, without resets", async () => {
    setUp([catalogueModel({ custom: true, listed: false, overrides: { display_name: "Claude Sonnet 4.5" } })]);
    await renderApp(projectPath("models?model=cm_sonnet"));

    const s = await sheet("Claude Sonnet 4.5");
    expect(await within(s).findByText("Custom")).toBeInTheDocument();
    expect(s).toHaveTextContent("Display nameClaude Sonnet 4.5set here");
    expect(within(s).queryByText("No longer listed")).not.toBeInTheDocument();
    expect(within(s).queryByRole("button", { name: "Reset to models.dev" })).not.toBeInTheDocument();
  });

  it("says when the model doesn't exist and goes back to the models", async () => {
    setUp();
    const { user, router } = await renderApp(projectPath("models?model=cm_missing"));

    const s = await sheet("Model");
    expect(await within(s).findByText("This model doesn't exist or was removed.")).toBeInTheDocument();
    await user.click(within(s).getByRole("button", { name: "Back to models" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("says when the model couldn't load", async () => {
    setUp();
    on("GET", ONE_MODEL, apiError(403, "permission_denied", "You can't see this model."));
    await renderApp(projectPath("models?model=cm_sonnet"));

    expect(await within(await sheet("Model")).findByText("Couldn't load this model.")).toBeInTheDocument();
    expect((await notifications().findAllByText("You can't see this model.")).length).toBeGreaterThan(0);
  });

  it("saves only the overrides that changed", async () => {
    setUp();
    on("PATCH", ONE_MODEL, sonnet());
    const { user } = await renderApp(projectPath("models?model=cm_sonnet"));

    const s = await sheet("Sonnet");
    await user.click(await within(s).findByRole("button", { name: "Edit overrides" }));
    expect(within(s).getByRole("button", { name: "Save overrides" })).toBeDisabled();
    const name = within(s).getByRole("textbox", { name: "Display name" });
    await user.clear(name);
    await user.type(name, "Sonnet fast");
    await user.click(within(s).getByRole("switch", { name: "Structured output" }));
    await user.click(within(s).getByRole("checkbox", { name: "PDF" }));
    await user.click(within(s).getByRole("switch", { name: "Reasoning" }));
    await user.click(within(s).getByRole("switch", { name: "Accepts temperature" }));
    await user.click(within(s).getByRole("button", { name: "Save overrides" }));

    expect(await notifications().findByText("Overrides saved")).toBeInTheDocument();
    expect(requests("PATCH", ONE_MODEL).map((r) => r.headers.get("if-match"))).toEqual(['"1"']);
    expect(requests("PATCH", ONE_MODEL).map((r) => r.body)).toEqual([
      { overrides: { display_name: "Sonnet fast", input_modalities: ["text", "image", "pdf"], reasoning: false, structured_output: true, temperature: false } },
    ]);
    expect(await within(s).findByText("Values in use")).toBeInTheDocument();
  });

  it("shows and changes whether the model accepts a forced tool choice", async () => {
    const opus = catalogueModel({ effective: { ...catalogueModel().effective, forced_tool_choice: false } });
    setUp([opus]);
    on("PATCH", ONE_MODEL, opus);
    const { user } = await renderApp(projectPath("models?model=cm_sonnet"));

    const s = await sheet("Claude Sonnet 4.5");
    expect(await within(s).findByText("Values in use")).toBeInTheDocument();
    expect(s).toHaveTextContent("Forced tool choiceNomodels.dev");
    await user.click(within(s).getByRole("button", { name: "Edit overrides" }));
    const forced = within(s).getByRole("switch", { name: "Accepts a forced tool choice" });
    expect(forced).not.toBeChecked();
    await user.click(forced);
    await user.click(within(s).getByRole("button", { name: "Save overrides" }));

    expect(await notifications().findByText("Overrides saved")).toBeInTheDocument();
    expect(requests("PATCH", ONE_MODEL).map((r) => r.body)).toEqual([{ overrides: { forced_tool_choice: true } }]);
  });

  it("shows and changes whether an Anthropic model checks where replayed thinking came from", async () => {
    const opus = catalogueModel({ effective: { ...catalogueModel().effective, thinking_binding: true } });
    setUp([opus]);
    on("PATCH", ONE_MODEL, opus);
    const { user } = await renderApp(projectPath("models?model=cm_sonnet"));

    const s = await sheet("Claude Sonnet 4.5");
    expect(await within(s).findByText("Values in use")).toBeInTheDocument();
    expect(s).toHaveTextContent("Thinking bindingYesmodels.dev");
    await user.click(within(s).getByRole("button", { name: "Edit overrides" }));
    const binding = within(s).getByRole("switch", { name: "Thinking binding" });
    expect(binding).toBeChecked();
    await user.click(binding);
    await user.click(within(s).getByRole("button", { name: "Save overrides" }));

    expect(await notifications().findByText("Overrides saved")).toBeInTheDocument();
    expect(requests("PATCH", ONE_MODEL).map((r) => r.body)).toEqual([{ overrides: { thinking_binding: false } }]);
  });

  it("explains values that can't be saved and cancels the edit", async () => {
    setUp();
    const { user } = await renderApp(projectPath("models?model=cm_sonnet"));

    const s = await sheet("Sonnet");
    await user.click(await within(s).findByRole("button", { name: "Edit overrides" }));
    const context = within(s).getByRole("textbox", { name: "Context" });
    await user.clear(context);
    await user.type(context, "1.5");
    await hoverReason(user, within(s).getByRole("button", { name: "Save overrides" }), "Limits and the timeout are whole numbers.");
    await user.clear(context);
    const price = within(s).getByRole("textbox", { name: "Output price" });
    await user.clear(price);
    await user.type(price, "2.");
    await hoverReason(user, within(s).getByRole("button", { name: "Save overrides" }), "Prices are decimal amounts in USD, like 3.00.");
    await user.click(within(s).getByRole("button", { name: "Cancel" }));
    expect(await within(s).findByText("Values in use")).toBeInTheDocument();
    expect(requests("PATCH", ONE_MODEL)).toHaveLength(0);
  });

  it("resets one field to models.dev", async () => {
    setUp();
    on("PATCH", ONE_MODEL, catalogueModel());
    const { user } = await renderApp(projectPath("models?model=cm_sonnet"));

    const s = await sheet("Sonnet");
    await within(s).findByText("Values in use");
    await user.click(within(s).getAllByRole("button", { name: "Reset to models.dev" })[1] as HTMLElement);
    expect(await notifications().findByText("Input price reset to models.dev")).toBeInTheDocument();
    expect(requests("PATCH", ONE_MODEL).map((r) => r.body)).toEqual([{ overrides: { input_price: null } }]);
  });

  it("shows only the reset in flight as busy", async () => {
    setUp();
    let answer: (m: unknown) => void = () => undefined;
    on("PATCH", ONE_MODEL, () => new Promise((resolve) => (answer = resolve)));
    const { user } = await renderApp(projectPath("models?model=cm_sonnet"));

    const s = await sheet("Sonnet");
    await within(s).findByText("Values in use");
    const [name, price] = within(s).getAllByRole("button", { name: "Reset to models.dev" });
    await user.click(price as HTMLElement);
    await waitFor(() => expect(price).toHaveAttribute("aria-busy", "true"));
    expect(name).not.toHaveAttribute("aria-busy");
    answer(catalogueModel());
    expect(await notifications().findByText("Input price reset to models.dev")).toBeInTheDocument();
  });

  it("shows a model of another provider by its id, with only the format the gateway sends it as", async () => {
    setUp([
      catalogueModel({
        id: "cm_acme",
        provider: "acme",
        model_id: "acme-large",
        egress_dialect: "openai_chat",
        effective: { ...catalogueModel().effective, display_name: null, input_modalities: ["text", "file"] },
      }),
    ]);
    const { user } = await renderApp(projectPath("models?model=cm_acme"));

    const s = await sheet("acme-large");
    expect(await within(s).findByText("Values in use")).toBeInTheDocument();
    expect(s).toHaveTextContent("Text, file");
    await user.click(within(s).getByRole("button", { name: "Edit overrides" }));
    await user.click(within(s).getByRole("combobox", { name: "Sends as" }));
    expect((await screen.findAllByRole("option")).map((o) => o.textContent)).toEqual(["OpenAI Chat Completions"]);
  });

  it("keeps the thinking setting of another provider's model that the gateway sends as Anthropic Messages", async () => {
    const model = catalogueModel({
      id: "cm_acme",
      provider: "acme",
      model_id: "acme-claude",
      egress_dialect: "anthropic_messages",
      effective: { ...catalogueModel().effective, display_name: null, adaptive_thinking: true, egress_dialect: null },
    });
    setUp([model]);
    on("PATCH", ONE_MODEL, model);
    const { user } = await renderApp(projectPath("models?model=cm_acme"));

    const s = await sheet("acme-claude");
    await user.click(await within(s).findByRole("button", { name: "Edit overrides" }));
    expect(within(s).getByRole("switch", { name: "Adaptive thinking" })).toBeChecked();
    await user.type(within(s).getByRole("textbox", { name: "Display name" }), "Acme Claude");
    await user.click(within(s).getByRole("button", { name: "Save overrides" }));

    expect(await notifications().findByText("Overrides saved")).toBeInTheDocument();
    expect(requests("PATCH", ONE_MODEL).map((r) => r.body)).toEqual([{ overrides: { display_name: "Acme Claude" } }]);
  });

  it("offers a Mistral model only the formats Mistral speaks", async () => {
    setUp([
      catalogueModel({
        id: "cm_small",
        provider: "mistral",
        model_id: "mistral-small-latest",
        egress_dialect: "openai_chat",
        effective: { ...catalogueModel().effective, display_name: null },
      }),
    ]);
    const { user } = await renderApp(projectPath("models?model=cm_small"));

    const s = await sheet("mistral-small-latest");
    expect(await within(s).findByText("Values in use")).toBeInTheDocument();
    await user.click(within(s).getByRole("button", { name: "Edit overrides" }));
    await user.click(within(s).getByRole("combobox", { name: "Sends as" }));
    expect((await screen.findAllByRole("option")).map((o) => o.textContent)).toEqual(["OpenAI Chat Completions", "OpenAI Embeddings"]);
  });

  it("says when someone else changed the model meanwhile, and reloads it", async () => {
    setUp();
    on("PATCH", ONE_MODEL, apiError(412, "conflict", "This catalogue model was changed since you loaded it; reload it and try again."));
    const { user } = await renderApp(projectPath("models?model=cm_sonnet"));

    const s = await sheet("Sonnet");
    await user.click(await within(s).findByRole("button", { name: "Edit overrides" }));
    const name = within(s).getByRole("textbox", { name: "Display name" });
    await user.clear(name);
    await user.type(name, "Sonnet fast");
    const loads = requests("GET", ONE_MODEL).length;
    await user.click(within(s).getByRole("button", { name: "Save overrides" }));

    const toasts = notifications();
    expect(await toasts.findByText("This catalogue model was changed since you loaded it; reload it and try again.")).toBeInTheDocument();
    expect(toasts.getByText("The latest version is loaded now.")).toBeInTheDocument();
    await waitFor(() => expect(requests("GET", ONE_MODEL).length).toBeGreaterThan(loads));
    expect(await within(s).findByRole("button", { name: "Edit overrides" })).toBeInTheDocument();
    expect(within(s).queryByRole("button", { name: "Save overrides" })).not.toBeInTheDocument();
  });

  it("keeps the edit and the removal open when they are refused for another reason", async () => {
    setUp();
    on("PATCH", ONE_MODEL, apiError(400, "invalid_request", "`overrides.display_name` is too long.", { param: "overrides.display_name" }));
    on("DELETE", ONE_MODEL, apiError(409, "conflict", "An alias still names this model; remove the alias first."));
    const { user } = await renderApp(projectPath("models?model=cm_sonnet"));

    const s = await sheet("Sonnet");
    await user.click(await within(s).findByRole("button", { name: "Edit overrides" }));
    const name = within(s).getByRole("textbox", { name: "Display name" });
    await user.clear(name);
    await user.type(name, "Sonnet fast");
    await user.click(within(s).getByRole("button", { name: "Save overrides" }));
    expect((await notifications().findByText("overrides.display_name", { selector: "code" })).parentElement).toHaveTextContent(
      /^overrides\.display_name is too long\.$/,
    );
    expect(within(s).getByRole("button", { name: "Save overrides" })).toBeInTheDocument();
    await user.click(within(s).getByRole("button", { name: "Cancel" }));

    await user.click(await within(s).findByRole("button", { name: "Model actions" }));
    await user.click(await screen.findByRole("menuitem", { name: "Remove from catalogue" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Remove Sonnet?" });
    await user.click(within(confirm).getByRole("button", { name: "Remove from catalogue" }));
    expect(await notifications().findByText("An alias still names this model; remove the alias first.")).toBeInTheDocument();
    expect(screen.getByRole("alertdialog", { name: "Remove Sonnet?" })).toBe(confirm);
    await user.click(within(confirm).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog", { name: "Remove Sonnet?" })).not.toBeInTheDocument());
    expect(screen.getByRole("dialog", { name: "Sonnet" })).toBeInTheDocument();
  });

  it("closes the removal when the model changed meanwhile", async () => {
    setUp();
    on("DELETE", ONE_MODEL, apiError(412, "conflict", "This catalogue model was changed since you loaded it; reload it and try again."));
    const { user } = await renderApp(projectPath("models?model=cm_sonnet"));

    const s = await sheet("Sonnet");
    await user.click(await within(s).findByRole("button", { name: "Model actions" }));
    await user.click(await screen.findByRole("menuitem", { name: "Remove from catalogue" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Remove Sonnet?" });
    await user.click(within(confirm).getByRole("button", { name: "Remove from catalogue" }));
    expect(await notifications().findByText("This catalogue model was changed since you loaded it; reload it and try again.")).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByRole("alertdialog", { name: "Remove Sonnet?" })).not.toBeInTheDocument());
    expect(screen.getByRole("dialog", { name: "Sonnet" })).toBeInTheDocument();
  });

  it("removes the model from the catalogue", async () => {
    setUp();
    on("DELETE", ONE_MODEL, () => undefined);
    const { user, router } = await renderApp(projectPath("models?model=cm_sonnet"));

    const s = await sheet("Sonnet");
    await user.click(await within(s).findByRole("button", { name: "Model actions" }));
    await user.click(await screen.findByRole("menuitem", { name: "Remove from catalogue" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Remove Sonnet?" });
    expect(within(confirm).getByText("Requests naming it are refused, and aliases and model lists that name it lose it.")).toBeInTheDocument();
    await user.click(within(confirm).getByRole("button", { name: "Remove from catalogue" }));
    expect(await notifications().findByText("Model removed from the catalogue")).toBeInTheDocument();
    expect(requests("DELETE", ONE_MODEL).map((r) => [r.params.model, r.headers.get("if-match")])).toEqual([["cm_sonnet", '"1"']]);
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("locks the overrides and removal without Manage models", async () => {
    setUp([sonnet()], without("models.manage"));
    const { user } = await renderApp(projectPath("models?model=cm_sonnet"));

    const s = await sheet("Sonnet");
    const edit = await within(s).findByRole("button", { name: "Edit overrides" });
    expect(edit).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, edit, /You need Manage models/);
    expect(within(s).getByRole("button", { name: "Model actions" })).toHaveAttribute("aria-disabled", "true");
    expect(within(s).queryByRole("button", { name: "Reset to models.dev" })).not.toBeInTheDocument();
  });
});
