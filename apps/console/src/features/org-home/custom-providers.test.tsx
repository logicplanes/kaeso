import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { CustomProviderView, ProviderView } from "~/api/generated/types";
import { ADMIN_ORG_RIGHTS, ORG, providers, signedIn, withRights } from "~/test/fixtures";
import { fill, hoverReason, notifications } from "~/test/fixtures/org";
import { catalogueModel, modelsApi, projectData, projectPath } from "~/test/fixtures/project";
import { apiError, noContent, on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { deleteConsequence, idFromName } from "./custom-providers";

const PATH = `/orgs/${ORG}/providers?tab=custom`;
const LIST = "orgs/:org/custom-providers";
const ONE = "orgs/:org/custom-providers/:provider";

function customProvider(overrides: Partial<CustomProviderView> = {}): CustomProviderView {
  return {
    id: "research-vllm",
    name: "Research vLLM",
    base_url: "https://vllm.example.com/",
    auth: "bearer",
    generation: "openai_chat",
    embeddings: null,
    count_tokens: null,
    hides_built_in: null,
    in_use: { keys: 1, models: 3, projects: 2 },
    created_by: { kind: "user", id: "usr_ada" },
    created_at: "2026-10-20T12:00:00Z",
    updated_at: "2026-10-20T12:00:00Z",
    revision: 4,
    ...overrides,
  };
}

function listed(p: CustomProviderView): ProviderView {
  return {
    id: p.id,
    name: p.name,
    custom: true,
    takes_key: p.auth !== "none",
    hides_built_in: p.hides_built_in ?? null,
    key_page: null,
    key_prefix: null,
    dialects: [p.generation, ...(p.embeddings ? [p.embeddings] : [])],
    count_tokens: p.count_tokens ?? null,
    embeddings: p.embeddings ?? null,
    account: null,
  };
}

function withCustom(list: CustomProviderView[]) {
  on("GET", LIST, page(list));
  on("GET", "orgs/:org/providers", page([...providers, ...list.map(listed)]));
  on("GET", "orgs/:org/credentials", { data: [] });
}

const table = () => screen.findByRole("table", { name: "Custom providers" });

describe("custom providers", () => {
  it("lists the organization's providers with their address, format, use and a built-in they replace", async () => {
    signedIn();
    withCustom([customProvider(), customProvider({ id: "groq", name: "Groq proxy", hides_built_in: "Groq", in_use: { keys: 0, models: 0, projects: 0 } })]);
    const { user } = await renderApp(PATH);

    const research = (await within(await table()).findByText("Research vLLM")).closest('[role="row"]') as HTMLElement;
    expect(research).toHaveTextContent("research-vllm");
    expect(research).toHaveTextContent("https://vllm.example.com/");
    expect(research).toHaveTextContent("OpenAI Chat Completions");
    expect(research).toHaveTextContent("1 key · 3 models");
    const proxy = (await within(await table()).findByText("Groq proxy")).closest('[role="row"]') as HTMLElement;
    expect(proxy).toHaveTextContent("Replaces Groq");
    expect(proxy).toHaveTextContent("Not used yet");
    expect(screen.getByRole("tab", { name: /Custom providers/ })).toHaveAttribute("aria-selected", "true");

    await user.click(screen.getByRole("tab", { name: /Provider keys/ }));
    expect(await screen.findByText("No provider keys yet")).toBeInTheDocument();
  });

  it("adds a provider, deriving its id from the name and sending only the formats it serves", async () => {
    signedIn();
    withCustom([]);
    on("POST", LIST, customProvider({ id: "lab-gpus", name: "Lab GPUs", in_use: { keys: 0, models: 0, projects: 0 } }));
    const { user } = await renderApp(PATH);

    await user.click(await screen.findByRole("button", { name: "Add custom provider" }));
    const dialog = await screen.findByRole("dialog", { name: "Add custom provider" });
    const add = () => within(dialog).getByRole("button", { name: "Add custom provider" });
    await hoverReason(user, add(), "Give the provider a name");
    await fill(user, within(dialog).getByRole("textbox", { name: "Name" }), "Lab GPUs");
    expect(within(dialog).getByRole("textbox", { name: "Id" })).toHaveValue("lab-gpus");
    await hoverReason(user, add(), "Give the base URL, starting with https://");
    await fill(user, within(dialog).getByRole("textbox", { name: "Base URL" }), "https://gpus.example.com");
    await user.click(within(dialog).getByRole("combobox", { name: "API format" }));
    await user.click(await screen.findByRole("option", { name: "Anthropic Messages" }));
    await user.click(within(dialog).getByRole("switch", { name: "Counts tokens" }));
    await user.click(within(dialog).getByRole("combobox", { name: "Key" }));
    await user.click(await screen.findByRole("option", { name: "x-api-key header" }));
    expect(within(dialog).getByText("Sent as x-api-key, as Anthropic-compatible servers expect.")).toBeInTheDocument();
    await user.click(add());

    expect(await notifications().findByText("Lab GPUs added")).toBeInTheDocument();
    expect(requests("POST", LIST).map((r) => r.body)).toEqual([
      {
        id: "lab-gpus",
        name: "Lab GPUs",
        base_url: "https://gpus.example.com",
        auth: "x_api_key",
        generation: "anthropic_messages",
        embeddings: null,
        count_tokens: "anthropic_messages",
      },
    ]);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Add custom provider" })).not.toBeInTheDocument());
  });

  it("shows the gateway's refusal of an address beside the base URL", async () => {
    signedIn();
    withCustom([]);
    on("POST", LIST, apiError(400, "invalid_request", "`base_url` points to an address this install does not allow.", { param: "base_url" }));
    const { user } = await renderApp(PATH);

    await user.click(await screen.findByRole("button", { name: "Add custom provider" }));
    const dialog = await screen.findByRole("dialog", { name: "Add custom provider" });
    await fill(user, within(dialog).getByRole("textbox", { name: "Name" }), "Metadata");
    await fill(user, within(dialog).getByRole("textbox", { name: "Base URL" }), "http://169.254.169.254");
    await user.click(within(dialog).getByRole("button", { name: "Add custom provider" }));

    const url = within(dialog).getByRole("textbox", { name: "Base URL" });
    await waitFor(() => expect(url).toHaveAccessibleDescription(/points to an address this install does not allow/));
  });

  it("changes a provider under its revision and never its id", async () => {
    signedIn();
    const research = customProvider();
    withCustom([research]);
    on("PATCH", ONE, customProvider({ name: "Research cluster", revision: 5 }));
    const { user } = await renderApp(PATH);

    await user.click(await screen.findByRole("button", { name: "Actions for Research vLLM" }));
    await user.click(await screen.findByRole("menuitem", { name: "Edit" }));
    const dialog = await screen.findByRole("dialog", { name: "Edit Research vLLM" });
    expect(within(dialog).getByRole("textbox", { name: "Id" })).toBeDisabled();
    const name = within(dialog).getByRole("textbox", { name: "Name" });
    await user.clear(name);
    await user.type(name, "Research cluster");
    await user.click(within(dialog).getByRole("switch", { name: "Also serves embeddings" }));
    await user.click(within(dialog).getByRole("button", { name: "Save" }));

    expect(await notifications().findByText("Research cluster saved")).toBeInTheDocument();
    const [sent] = requests("PATCH", ONE);
    expect(sent?.headers.get("if-match")).toBe('"4"');
    expect(sent?.body).toEqual({
      name: "Research cluster",
      base_url: "https://vllm.example.com/",
      auth: "bearer",
      generation: "openai_chat",
      embeddings: "openai_embeddings",
      count_tokens: null,
    });
  });

  it("says what deleting removes before it deletes", async () => {
    signedIn();
    withCustom([customProvider()]);
    on("DELETE", ONE, noContent);
    const { user } = await renderApp(PATH);

    await user.click(await screen.findByRole("button", { name: "Actions for Research vLLM" }));
    await user.click(await screen.findByRole("menuitem", { name: "Delete" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Delete Research vLLM?" });
    expect(confirm).toHaveTextContent("Its 1 key and 3 models in 2 projects go too; requests to them fail.");
    await user.click(within(confirm).getByRole("button", { name: "Delete provider" }));

    expect(await notifications().findByText("Research vLLM deleted")).toBeInTheDocument();
    expect(requests("DELETE", ONE)[0]?.headers.get("if-match")).toBe('"4"');
    expect(deleteConsequence(customProvider({ in_use: { keys: 0, models: 0, projects: 0 } }))).toBe("Nothing uses it yet; requests are not affected.");
  });

  it("hides the tab from members who can't see custom providers and explains a missing manage right", async () => {
    withRights(ADMIN_ORG_RIGHTS.filter((r) => !r.startsWith("providers.custom")));
    withCustom([customProvider()]);
    await renderApp(PATH);
    expect(await screen.findByRole("table", { name: "Provider keys" })).toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: /Custom providers/ })).not.toBeInTheDocument();
    expect(requests("GET", LIST)).toEqual([]);
  });

  it("asks for the manage right before adding a provider", async () => {
    withRights(ADMIN_ORG_RIGHTS.filter((r) => r !== "providers.custom.manage"));
    withCustom([]);
    const { user } = await renderApp(PATH);
    const add = await screen.findByRole("button", { name: "Add custom provider" });
    await hoverReason(user, add, "You need Manage custom providers; ask an owner or admin for it.");
    expect(screen.queryByRole("dialog", { name: "Add custom provider" })).not.toBeInTheDocument();
  });

  it("offers the add action from the header, the create shortcut and the tab", async () => {
    signedIn();
    withCustom([customProvider()]);
    const { user } = await renderApp(`/orgs/${ORG}/providers`);

    await user.click(await screen.findByRole("tab", { name: /Custom providers/ }));
    expect(await within(await table()).findByText("Research vLLM")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Add custom provider" }));
    const dialog = await screen.findByRole("dialog", { name: "Add custom provider" });
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Add custom provider" })).not.toBeInTheDocument());

    await user.keyboard("c");
    expect(await screen.findByRole("dialog", { name: "Add custom provider" })).toBeInTheDocument();
  });

  it("takes an id typed by hand, explains each key style and clears a refusal once the field changes", async () => {
    signedIn();
    withCustom([]);
    on("POST", LIST, apiError(400, "invalid_request", "`lmstudio` is a provider listed on models.dev that Kaeso may add; choose another id.", { param: "id" }));
    const { user } = await renderApp(PATH);

    await user.click(await screen.findByRole("button", { name: "Add custom provider" }));
    const dialog = await screen.findByRole("dialog", { name: "Add custom provider" });
    await fill(user, within(dialog).getByRole("textbox", { name: "Name" }), "LM Studio");
    await fill(user, within(dialog).getByRole("textbox", { name: "Base URL" }), "http://10.0.0.8:1234");
    const id = within(dialog).getByRole("textbox", { name: "Id" });
    await user.clear(id);
    await user.type(id, "My Lab");
    await hoverReason(
      user,
      within(dialog).getByRole("button", { name: "Add custom provider" }),
      "Give the provider an id of lowercase letters, digits and hyphens",
    );
    await user.clear(id);
    await user.type(id, "LMSTUDIO");
    expect(id).toHaveValue("lmstudio");
    await user.click(within(dialog).getByRole("combobox", { name: "Key" }));
    await user.click(await screen.findByRole("option", { name: "No key" }));
    expect(within(dialog).getByText("Requests go out without a key, for a server on your own network.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Add custom provider" }));
    await waitFor(() => expect(id).toHaveAccessibleDescription(/listed on models.dev/));
    await user.type(id, "-local");
    await waitFor(() => expect(id).not.toHaveAccessibleDescription(/listed on models.dev/));
    expect(requests("POST", LIST)[0]?.body).toMatchObject({ id: "lmstudio", auth: "none", base_url: "http://10.0.0.8:1234" });
  });

  it("says a provider's keys go first before it takes no key", async () => {
    signedIn();
    withCustom([customProvider()]);
    const { user } = await renderApp(PATH);

    await user.click(await screen.findByRole("button", { name: "Actions for Research vLLM" }));
    await user.click(await screen.findByRole("menuitem", { name: "Edit" }));
    const dialog = await screen.findByRole("dialog", { name: "Edit Research vLLM" });
    expect(within(dialog).queryByText(/Delete its keys under Provider keys first/)).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("combobox", { name: "Key" }));
    await user.click(await screen.findByRole("option", { name: "No key" }));
    expect(within(dialog).getByText("Delete its keys under Provider keys first; a provider without a key can't keep any.")).toBeInTheDocument();
  });

  it("closes the confirmation when the provider changed in the meantime and reloads it", async () => {
    signedIn();
    withCustom([customProvider()]);
    on("DELETE", ONE, apiError(412, "conflict", "This custom provider was changed since you loaded it; reload it and try again."));
    const { user } = await renderApp(PATH);

    await user.click(await screen.findByRole("button", { name: "Actions for Research vLLM" }));
    await user.click(await screen.findByRole("menuitem", { name: "Delete" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Delete Research vLLM?" });
    await user.click(within(confirm).getByRole("button", { name: "Delete provider" }));
    expect(await notifications().findByText("This custom provider was changed since you loaded it; reload it and try again.")).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(requests("GET", LIST).length).toBeGreaterThan(1);
  });

  it("shows viewers the providers without actions and says when the list can't load", async () => {
    withRights(ADMIN_ORG_RIGHTS.filter((r) => r !== "providers.custom.manage"));
    withCustom([customProvider()]);
    await renderApp(PATH);
    expect(await within(await table()).findByText("Research vLLM")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Actions for Research vLLM" })).not.toBeInTheDocument();
  });

  it("offers to load the list again when it fails", async () => {
    signedIn();
    withCustom([]);
    let calls = 0;
    on("GET", LIST, () => (++calls === 1 ? apiError(400, "invalid_request", "The custom providers are locked.") : page([customProvider()])));
    const { user } = await renderApp(PATH);
    expect(await within(await table()).findByText("Couldn't load these rows")).toBeInTheDocument();
    await user.click(within(await table()).getByRole("button", { name: "Try again" }));
    expect(await within(await table()).findByText("Research vLLM")).toBeInTheDocument();
  });

  it("folds the address, format and use under the name once their columns give way", async () => {
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
      width: 360,
      height: 0,
      top: 0,
      left: 0,
      right: 360,
      bottom: 0,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    });
    try {
      signedIn();
      withCustom([customProvider()]);
      await renderApp(PATH);
      const headers = within(await table())
        .getAllByRole("columnheader")
        .map((h) => h.textContent);
      expect(headers).not.toContain("Base URL");
      expect(
        await within(await table()).findByText(
          (_, el) =>
            el?.textContent === "research-vllm · Base URL: https://vllm.example.com/ · API format: OpenAI Chat Completions · In use: 1 key · 3 models" &&
            el.tagName !== "DIV",
        ),
      ).toBeInTheDocument();
    } finally {
      vi.restoreAllMocks();
    }
  });

  it("derives ids the gateway accepts from any name", () => {
    expect(idFromName("  Research vLLM (GPU) ")).toBe("research-vllm-gpu");
    expect(idFromName("Ünïcode Lab")).toBe("unicode-lab");
    expect(idFromName("a".repeat(50))).toHaveLength(40);
  });
});

describe("a custom provider's models", () => {
  it("offers the ids it lists and takes every value by hand", async () => {
    signedIn();
    projectData();
    modelsApi({ models: [catalogueModel()] });
    const research = customProvider();
    on("GET", "orgs/:org/providers", page([...providers, listed(research)]));
    on("GET", "orgs/:org/projects/:project/models/offered", {
      data: [
        { model_id: "meta-llama/Llama-3.3-70B", catalogue_model_id: null },
        { model_id: "qwen/qwen3-32b", catalogue_model_id: "cm_qwen" },
      ],
    });
    on(
      "POST",
      "orgs/:org/projects/:project/models",
      catalogueModel({ id: "cm_llama", provider: "research-vllm", model_id: "meta-llama/Llama-3.3-70B", custom: true }),
    );
    const { user } = await renderApp(projectPath("models?add=true"));
    const dialog = await screen.findByRole("dialog", { name: /Add models/ });
    await user.click(within(dialog).getByRole("button", { description: "Provider" }));
    await user.click(await screen.findByRole("option", { name: /^Research vLLM/ }));

    expect(await within(dialog).findByText("meta-llama/Llama-3.3-70B")).toBeInTheDocument();
    expect(within(dialog).queryByText("qwen/qwen3-32b")).not.toBeInTheDocument();
    expect(within(dialog).queryByRole("switch", { name: "Include deprecated" })).not.toBeInTheDocument();
    expect(requests("GET", "orgs/:org/projects/:project/models/search").filter((r) => r.query.get("provider") === "research-vllm")).toEqual([]);
    await user.click(within(dialog).getByRole("gridcell", { name: /Llama-3.3-70B/ }));

    const custom = await screen.findByRole("dialog", { name: /Add a Research vLLM model/ });
    expect(within(custom).getByText(/Kaeso can't know these values for Research vLLM/)).toBeInTheDocument();
    expect(within(custom).queryByText("Saved values override models.dev for this project only.")).not.toBeInTheDocument();
    await user.click(within(custom).getAllByRole("checkbox", { name: "Text" })[0] as HTMLElement);
    await user.click(within(custom).getAllByRole("checkbox", { name: "Text" })[1] as HTMLElement);
    await fill(user, within(custom).getByRole("textbox", { name: "Context" }), "131072");
    await fill(user, within(custom).getByRole("textbox", { name: "Output limit" }), "8192");
    await fill(user, within(custom).getByRole("textbox", { name: "Input price" }), "0.2");
    await fill(user, within(custom).getByRole("textbox", { name: "Output price" }), "0.6");
    await user.click(within(custom).getByRole("button", { name: "Add model" }));

    expect(await notifications().findByText("Added meta-llama/Llama-3.3-70B")).toBeInTheDocument();
    expect(requests("POST", "orgs/:org/projects/:project/models").map((r) => r.body)).toEqual([
      {
        provider: "research-vllm",
        model_id: "meta-llama/Llama-3.3-70B",
        custom: true,
        overrides: {
          input_modalities: ["text"],
          output_modalities: ["text"],
          context_limit: 131072,
          output_limit: 8192,
          input_price: "0.2",
          output_price: "0.6",
          egress_dialect: "openai_chat",
          tool_call: false,
          reasoning: false,
          structured_output: false,
          temperature: false,
        },
      },
    ]);
  });

  it("falls back to typing an id when the provider lists nothing", async () => {
    signedIn();
    projectData();
    modelsApi({ models: [] });
    const research = customProvider({ auth: "none" });
    on("GET", "orgs/:org/providers", page([...providers, listed(research)]));
    on(
      "GET",
      "orgs/:org/projects/:project/models/offered",
      apiError(404, "not_found", "The custom provider `research-vllm` has no model listing; type the model id instead."),
    );
    const { user } = await renderApp(projectPath("models?add=true"));
    const dialog = await screen.findByRole("dialog", { name: /Add models/ });
    await user.click(within(dialog).getByRole("button", { description: "Provider" }));
    await user.click(await screen.findByRole("option", { name: /^Research vLLM/ }));

    expect(await within(dialog).findByText("The custom provider `research-vllm` has no model listing; type the model id instead.")).toBeInTheDocument();
    expect(within(dialog).queryByText(/fail until someone adds a key/)).not.toBeInTheDocument();
    await user.click(within(dialog).getAllByRole("button", { name: "Type a model id" })[0] as HTMLElement);
    expect(await screen.findByRole("dialog", { name: /Add a Research vLLM model/ })).toBeInTheDocument();
  });

  it("narrows the listed ids by the search, says when none match, and takes an id typed by hand", async () => {
    signedIn();
    projectData();
    modelsApi({ models: [] });
    const research = customProvider();
    on("GET", "orgs/:org/providers", page([...providers, listed(research)]));
    on("GET", "orgs/:org/projects/:project/models/offered", { data: [{ model_id: "meta-llama/Llama-3.3-70B", catalogue_model_id: null }] });
    const { user } = await renderApp(projectPath("models?add=true"));
    const dialog = await screen.findByRole("dialog", { name: /Add models/ });
    await user.click(within(dialog).getByRole("button", { description: "Provider" }));
    await user.click(await screen.findByRole("option", { name: /^Research vLLM/ }));
    expect(await within(dialog).findByText("meta-llama/Llama-3.3-70B")).toBeInTheDocument();

    await user.type(within(dialog).getByRole("searchbox", { name: "Search models" }), "qwen");
    expect(await within(dialog).findByText("Research vLLM lists no model with that name; type its id instead.")).toBeInTheDocument();
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    const custom = await screen.findByRole("dialog", { name: /Add a Research vLLM model/ });
    const id = within(custom).getByRole("combobox", { name: "Model id" });
    await user.type(id, "qwen3-32b");
    expect(within(custom).getByText(/Kaeso can't know these values for Research vLLM/)).toBeInTheDocument();
    expect(within(custom).getByText("OpenAI Chat Completions")).toBeInTheDocument();
  });

  it("says when every model the provider lists is in the catalogue", async () => {
    signedIn();
    projectData();
    modelsApi({ models: [] });
    const research = customProvider();
    on("GET", "orgs/:org/providers", page([...providers, listed(research)]));
    on("GET", "orgs/:org/projects/:project/models/offered", { data: [{ model_id: "qwen/qwen3-32b", catalogue_model_id: "cm_qwen" }] });
    const { user } = await renderApp(projectPath("models?add=true"));
    const dialog = await screen.findByRole("dialog", { name: /Add models/ });
    await user.click(within(dialog).getByRole("button", { description: "Provider" }));
    await user.click(await screen.findByRole("option", { name: /^Research vLLM/ }));
    expect(await within(dialog).findByText("Every model Research vLLM lists is in the catalogue already.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Type a model id" }));
    expect(await screen.findByRole("dialog", { name: /Add a Research vLLM model/ })).toBeInTheDocument();
  });

  it("leaves a provider without a key out of the provider key dialog", async () => {
    signedIn();
    const open = customProvider({ id: "open-lab", name: "Open lab", auth: "none" });
    const keyed = customProvider({ id: "keyed-lab", name: "Keyed lab" });
    on("GET", "orgs/:org/providers", page([...providers, listed(open), listed(keyed)]));
    on("GET", "orgs/:org/credentials", { data: [] });
    const { user } = await renderApp(`/orgs/${ORG}/providers`);

    await user.click((await screen.findAllByRole("button", { name: "Add provider key" }))[0] as HTMLElement);
    const dialog = await screen.findByRole("dialog", { name: /Add provider key/ });
    expect(within(dialog).queryByRole("option", { name: /^Open lab/ })).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("option", { name: /^Keyed lab/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByText("Paste the key that Keyed lab expects.")).toBeInTheDocument();
  });
});
