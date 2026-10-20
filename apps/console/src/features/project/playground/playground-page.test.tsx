import { describe, expect, it, vi } from "vitest";
import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { ORG, PROJECT, signedIn, without } from "~/test/fixtures";
import { hoverReason, notifications } from "~/test/fixtures/org";
import { apiError, on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { eventStream, playgroundModel, sse } from "~/test/fixtures/project-logs";

const MODELS = "orgs/:org/projects/:project/playground/models";
const RESPONSES = "orgs/:org/projects/:project/playground/responses";
const PLAYGROUND = `/orgs/${ORG}/projects/${PROJECT}/playground`;
const claude = playgroundModel({
  id: "cm_c",
  provider: "anthropic",
  model: "claude",
  model_id: "claude-opus-5-5",
  display_name: "Claude Opus 5.5",
  reasoning: true,
  efforts: ["low", "high"],
  temperature: false,
});
const reply = (text: string) =>
  eventStream(
    sse([
      { type: "response.created" },
      { type: "response.output_text.delta", item_id: "m", delta: text },
      { type: "response.completed", response: { usage: { input_tokens: 3, output_tokens: 2 } } },
    ]),
  );
const pointer = (fine: boolean) =>
  vi.spyOn(window, "matchMedia").mockImplementation(
    (query: string) =>
      ({
        matches: fine && query === "(pointer: fine)",
        media: query,
        addEventListener: () => undefined,
        removeEventListener: () => undefined,
      }) as unknown as MediaQueryList,
  );
const sentBodies = () => requests("POST", RESPONSES).map((r) => r.body as Record<string, unknown>);

describe("PlaygroundPage", () => {
  it("puts the cursor in the prompt once the models are ready, with a mouse or trackpad", async () => {
    const media = pointer(true);
    signedIn();
    on("GET", MODELS, page([playgroundModel()]));
    await renderApp(PLAYGROUND);
    await waitFor(() => expect(screen.getByRole("textbox", { name: "Prompt" })).toHaveFocus());
    media.mockRestore();
  });

  it("leaves the prompt alone on a touch screen", async () => {
    const media = pointer(false);
    signedIn();
    on("GET", MODELS, page([playgroundModel()]));
    await renderApp(PLAYGROUND);
    await waitFor(() => expect(screen.getByRole("textbox", { name: "Prompt" })).toBeEnabled());
    expect(screen.getByRole("textbox", { name: "Prompt" })).not.toHaveFocus();
    media.mockRestore();
  });

  it("sends a prompt and streams the model's reply", async () => {
    signedIn();
    on("GET", MODELS, page([playgroundModel(), claude]));
    on("POST", RESPONSES, () => reply("Hello."));
    const { user } = await renderApp(PLAYGROUND);
    await screen.findByText("claude-opus-5-5");
    await user.type(screen.getByRole("textbox", { name: "Prompt" }), "Hi");
    await user.click(screen.getByRole("button", { name: "Send" }));
    expect(await screen.findByText("Hello.")).toBeInTheDocument();
    const sent = requests("POST", RESPONSES);
    expect(sent.length).toBeGreaterThan(0);
    expect(JSON.stringify(sent[0]?.body)).toContain("Hi");
  });

  it("offers to add models when the project has none to try", async () => {
    signedIn();
    on("GET", MODELS, page([]));
    await renderApp(PLAYGROUND);

    expect(await screen.findByText("No models to try yet")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Add models" })).toHaveAttribute("href", `${PLAYGROUND.replace("/playground", "")}/models?add=true`);
    expect(screen.getByRole("button", { name: "Add column" })).toBeDisabled();
  });

  it("doesn't offer to add models without the right", async () => {
    signedIn({ projectRights: without("models.manage") });
    on("GET", MODELS, page([]));
    await renderApp(PLAYGROUND);

    expect(await screen.findByText("No models to try yet")).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "Add models" })).not.toBeInTheDocument();
  });

  it("adds columns up to the limit, sends to each and removes one", async () => {
    signedIn();
    on(
      "GET",
      MODELS,
      page([playgroundModel(), claude, playgroundModel({ id: "cm_n", model_id: "gpt-5.4-nano", display_name: "GPT-5.4 nano", model: "gpt-5.4-nano" })]),
    );
    on("POST", RESPONSES, ({ body }: { body: unknown }) => reply(`From ${(body as { model: string }).model}.`));
    const { user, router } = await renderApp(PLAYGROUND);

    await screen.findByText("claude-opus-5-5");
    await user.click(screen.getByRole("button", { name: "Add column" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ models: "anthropic/claude-opus-5-5,openai/gpt-5.4-mini" }));
    await user.click(screen.getByRole("button", { name: "Add column" }));
    await user.click(screen.getByRole("button", { name: "Add column" }));
    await waitFor(() => expect(screen.getAllByRole("button", { name: "Model" })).toHaveLength(4));
    const add = screen.getByRole("button", { name: "Add column" });
    expect(add).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, add, "Compare up to 4 models at once");

    expect(screen.getByRole("textbox", { name: "Prompt" })).toHaveAttribute("placeholder", "Send a prompt to every model");
    await user.type(screen.getByRole("textbox", { name: "Prompt" }), "Hi");
    await user.click(screen.getByRole("button", { name: "Send" }));
    expect(await screen.findAllByText("From claude.")).toHaveLength(2);
    await waitFor(() => expect(sentBodies()).toHaveLength(4));

    await user.click(screen.getAllByRole("button", { name: "Remove Claude Opus 5.5" })[0] as HTMLElement);
    await waitFor(() => expect(screen.getAllByRole("button", { name: "Model" })).toHaveLength(3));
    expect(screen.getAllByText("From claude.")).toHaveLength(1);
    expect(screen.getByText("From gpt-5.4-mini.")).toBeInTheDocument();
    expect(router.state.location.search).toEqual({ models: "openai/gpt-5.4-mini,openai/gpt-5.4-nano,anthropic/claude-opus-5-5" });
  });

  it("switches a column's model and marks models without a provider key", async () => {
    signedIn();
    on(
      "GET",
      MODELS,
      page([
        playgroundModel(),
        playgroundModel({ id: "cm_x", provider: "anthropic", model_id: "claude-x", display_name: "Claude X", provider_key: false }),
        claude,
      ]),
    );
    const { user, router } = await renderApp(PLAYGROUND);

    await screen.findByText("claude-opus-5-5");
    await user.click(screen.getByRole("button", { name: "Model" }));
    const listbox = await screen.findByRole("listbox");
    expect(within(listbox).getByRole("option", { name: /Claude X claude-x No Anthropic key/ })).toHaveAttribute("aria-disabled", "true");
    await user.click(within(listbox).getByRole("option", { name: /GPT-5.4 mini/ }));
    await user.click(screen.getByRole("button", { name: "Model" }));
    const recent = within(await screen.findByRole("listbox")).getByRole("group", { name: "Recent" });
    expect(within(recent).getByRole("option", { name: /GPT-5.4 mini/ })).toHaveAttribute("aria-selected", "true");
    await user.click(within(screen.getByRole("group", { name: "Anthropic" })).getByRole("option", { name: /Claude Opus 5.5/ }));
    await waitFor(() => expect(router.state.location.search).toEqual({ models: "anthropic/claude-opus-5-5" }));
  });

  it("keeps the model just chosen when a column is added before the page catches up", async () => {
    signedIn();
    on(
      "GET",
      MODELS,
      page([playgroundModel(), claude, playgroundModel({ id: "cm_n", model_id: "gpt-5.4-nano", display_name: "GPT-5.4 nano", model: "gpt-5.4-nano" })]),
    );
    const { user, router } = await renderApp(PLAYGROUND);

    await screen.findByText("claude-opus-5-5");
    await user.click(screen.getByRole("button", { name: "Model" }));
    const nano = within(await screen.findByRole("listbox")).getByRole("option", { name: /GPT-5.4 nano/ });
    act(() => {
      fireEvent.click(nano);
      fireEvent.click(screen.getByRole("button", { name: "Add column" }));
    });
    await waitFor(() => expect(router.state.location.search).toEqual({ models: "openai/gpt-5.4-nano,anthropic/claude-opus-5-5" }));
  });

  it("greys out models the provider doesn't serve and never adds one", async () => {
    signedIn();
    on(
      "GET",
      MODELS,
      page([
        playgroundModel({ id: "cm_g", provider: "groq", model_id: "llama-3.1-8b-instant", display_name: "Llama 3.1 8B", availability: "unavailable" }),
        playgroundModel(),
      ]),
    );
    const { user, router } = await renderApp(PLAYGROUND);

    expect(await screen.findByText("gpt-5.4-mini")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Model" }));
    const llama = within(await screen.findByRole("listbox")).getByRole("option", { name: /Llama 3.1 8B/ });
    expect(llama).toHaveAttribute("aria-disabled", "true");
    expect(llama).toHaveTextContent("The provider doesn't serve it");
    await user.keyboard("{Escape}{Escape}");
    await user.click(screen.getByRole("button", { name: "Add column" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ models: "openai/gpt-5.4-mini,openai/gpt-5.4-mini" }));
  });

  it("asks for a model when the link names one the project doesn't have", async () => {
    signedIn();
    on("GET", MODELS, page([playgroundModel()]));
    await renderApp(`${PLAYGROUND}?models=openai/gone-model`);

    const select = await screen.findByRole("button", { name: "Model" });
    expect(select).toHaveTextContent("Choose a model");
    expect(screen.getByRole("textbox", { name: "Prompt" })).toBeDisabled();
  });

  it("sends the parameters from the link and keeps them in step with it", async () => {
    signedIn();
    on("GET", MODELS, page([claude]));
    on("POST", RESPONSES, () => reply("Brief."));
    const { user, router } = await renderApp(`${PLAYGROUND}?system=Be%20brief.&max_tokens=200&effort=high&summary=concise`);

    await screen.findByText("claude-opus-5-5");
    await user.type(screen.getByRole("textbox", { name: "Prompt" }), "Hi");
    await user.click(screen.getByRole("button", { name: "Send" }));
    await screen.findByText("Brief.");
    expect(sentBodies()[0]).toMatchObject({ instructions: "Be brief.", max_output_tokens: 200, reasoning: { effort: "high", summary: "concise" } });

    await router.navigate({ to: "/orgs/$org/projects/$project/playground", params: { org: ORG, project: PROJECT }, search: { system: "Be kind." } });
    await user.click(screen.getByRole("button", { name: "Parameters" }));
    const dialog = await screen.findByRole("dialog", { name: "Parameters" });
    expect(within(dialog).getByRole("textbox", { name: "System instructions" })).toHaveValue("Be kind.");
    await user.clear(within(dialog).getByRole("textbox", { name: "System instructions" }));
    await user.type(within(dialog).getByRole("textbox", { name: "System instructions" }), "Be terse.");
    await waitFor(() => expect(router.state.location.search).toEqual({ system: "Be terse." }));
  });

  it("fills the prompt from a suggestion and keeps the draft", async () => {
    signedIn();
    on("GET", MODELS, page([playgroundModel()]));
    window.localStorage.setItem(`kaeso:playground-draft:${PROJECT}`, "Unsent question");
    const { user } = await renderApp(PLAYGROUND);

    const prompt = await screen.findByRole("textbox", { name: "Prompt" });
    expect(prompt).toHaveValue("Unsent question");
    await user.click(screen.getByRole("button", { name: "Write a haiku about latency" }));
    expect(prompt).toHaveValue("Write a haiku about latency");
    expect(window.localStorage.getItem(`kaeso:playground-draft:${PROJECT}`)).toBe("Write a haiku about latency");
  });

  it("retries a failed reply and clears the threads", async () => {
    signedIn();
    on("GET", MODELS, page([playgroundModel()]));
    let calls = 0;
    on("POST", RESPONSES, () =>
      ++calls === 1
        ? eventStream(sse([{ type: "response.created" }, { type: "response.failed", response: { error: { message: "Model fell over." } } }]))
        : reply("Second try worked."),
    );
    const { user } = await renderApp(PLAYGROUND);

    await screen.findByText("gpt-5.4-mini");
    expect(screen.getByText("Nothing sent yet")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Clear" })).not.toBeInTheDocument();
    await user.type(screen.getByRole("textbox", { name: "Prompt" }), "Hi");
    await user.click(screen.getByRole("button", { name: "Send" }));
    const failure = await screen.findByRole("alert");
    expect(failure).toHaveTextContent("No response");
    expect(failure).toHaveTextContent("Model fell over.");
    expect(notifications().queryByText("Model fell over.")).not.toBeInTheDocument();
    await user.click(await screen.findByRole("button", { name: "Retry" }));
    expect(await screen.findByText("Second try worked.")).toBeInTheDocument();
    expect(sentBodies()).toHaveLength(2);
    expect(sentBodies()[1]).toEqual(sentBodies()[0]);

    await user.click(screen.getByRole("button", { name: "Clear" }));
    await waitFor(() => expect(screen.queryByText("Second try worked.")).not.toBeInTheDocument());
    expect(screen.getByRole("button", { name: "Write a haiku about latency" })).toBeInTheDocument();
    expect(await notifications().findByText("Conversation cleared")).toBeInTheDocument();
    await user.click(notifications().getByRole("button", { name: "Undo" }));
    expect(await screen.findByText("Second try worked.")).toBeInTheDocument();
    expect(screen.getByText("Hi")).toBeInTheDocument();
  });

  it("keeps the conversation when you leave the Playground and come back, and asks before the tab closes", async () => {
    signedIn();
    on(
      "GET",
      MODELS,
      page([playgroundModel(), claude, playgroundModel({ id: "cm_n", model_id: "gpt-5.4-nano", display_name: "GPT-5.4 nano", model: "gpt-5.4-nano" })]),
    );
    on("POST", RESPONSES, ({ body }: { body: unknown }) => reply(`From ${(body as { model: string }).model}.`));
    const { user, router } = await renderApp(`${PLAYGROUND}?models=openai/gpt-5.4-nano,openai/gpt-5.4-mini`);

    await screen.findByText("gpt-5.4-nano");
    const leaving = () => {
      const e = new Event("beforeunload", { cancelable: true });
      window.dispatchEvent(e);
      return e.defaultPrevented;
    };
    expect(leaving()).toBe(false);
    await user.type(screen.getByRole("textbox", { name: "Prompt" }), "Hi");
    await user.click(screen.getByRole("button", { name: "Send" }));
    expect(await screen.findByText("From gpt-5.4-nano.")).toBeInTheDocument();
    expect(await screen.findByText("From gpt-5.4-mini.")).toBeInTheDocument();
    expect(leaving()).toBe(true);

    await router.navigate({ to: "/orgs/$org/projects/$project/keys", params: { org: ORG, project: PROJECT } });
    await waitFor(() => expect(screen.queryByText("From gpt-5.4-nano.")).not.toBeInTheDocument());
    expect(leaving()).toBe(false);
    await router.navigate({ to: "/orgs/$org/projects/$project/playground", params: { org: ORG, project: PROJECT } });
    expect(await screen.findByText("From gpt-5.4-nano.")).toBeInTheDocument();
    expect(screen.getByText("From gpt-5.4-mini.")).toBeInTheDocument();
    await waitFor(() => expect(router.state.location.search).toEqual({ models: "openai/gpt-5.4-nano,openai/gpt-5.4-mini" }));
    expect(leaving()).toBe(true);

    await user.click(screen.getByRole("button", { name: "Clear" }));
    await router.navigate({ to: "/orgs/$org/projects/$project/keys", params: { org: ORG, project: PROJECT } });
    await router.navigate({ to: "/orgs/$org/projects/$project/playground", params: { org: ORG, project: PROJECT } });
    expect(await screen.findByText("Nothing sent yet")).toBeInTheDocument();
  });

  it("sends the earlier turns with a follow-up", async () => {
    signedIn();
    on("GET", MODELS, page([playgroundModel()]));
    let calls = 0;
    on("POST", RESPONSES, () => reply(++calls === 1 ? "First answer." : "Second answer."));
    const { user } = await renderApp(PLAYGROUND);

    await screen.findByText("gpt-5.4-mini");
    await user.type(screen.getByRole("textbox", { name: "Prompt" }), "One");
    await user.click(screen.getByRole("button", { name: "Send" }));
    await screen.findByText("First answer.");
    await user.type(screen.getByRole("textbox", { name: "Prompt" }), "Two");
    await user.click(screen.getByRole("button", { name: "Send" }));
    await screen.findByText("Second answer.");
    expect(JSON.stringify(sentBodies()[1]?.input)).toContain("First answer.");
    expect(screen.getAllByRole("article", { name: "GPT-5.4 mini response" })).toHaveLength(2);
  });

  it("shows how long a model reasoned and offers every effort when it lists none", async () => {
    signedIn();
    const thinker = playgroundModel({ id: "cm_t", model: "thinker", model_id: "thinker-1", display_name: "Thinker", reasoning: true, efforts: [] });
    on("GET", MODELS, page([thinker]));
    on("POST", RESPONSES, () =>
      eventStream(
        sse([
          { type: "response.created" },
          { type: "response.reasoning_summary_text.delta", item_id: "rs", summary_index: 0, delta: "Plan the answer." },
          { type: "response.output_text.delta", item_id: "m", delta: "Answer." },
          { type: "response.completed", response: { usage: { input_tokens: 3, output_tokens: 2 } } },
        ]),
      ),
    );
    const { user } = await renderApp(PLAYGROUND);

    await screen.findByText("thinker-1");
    await user.type(screen.getByRole("textbox", { name: "Prompt" }), "Hi");
    await user.click(screen.getByRole("button", { name: "Send" }));
    const answer = await screen.findByRole("article", { name: "Thinker response" });
    await waitFor(() => expect(answer).toHaveTextContent(/Thought for/));
    expect(answer).toHaveTextContent("Answer.");

    await user.click(screen.getByRole("button", { name: "Parameters" }));
    const dialog = await screen.findByRole("dialog", { name: "Parameters" });
    await user.click(within(dialog).getByRole("combobox", { name: "Reasoning effort" }));
    expect(await screen.findByRole("option", { name: "Max" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "None" })).toBeInTheDocument();
  });

  it("stops a reply and keeps what arrived", async () => {
    signedIn();
    on("GET", MODELS, page([playgroundModel()]));
    on("POST", RESPONSES, () => {
      const head = sse([{ type: "response.created" }, { type: "response.output_text.delta", item_id: "m", delta: "Partial answer" }]);
      return new Response(
        new ReadableStream<Uint8Array>({
          start: (c) => c.enqueue(new TextEncoder().encode(head)),
        }),
        { status: 200, headers: { "content-type": "text/event-stream" } },
      );
    });
    const { user } = await renderApp(PLAYGROUND);

    await screen.findByText("gpt-5.4-mini");
    await user.type(screen.getByRole("textbox", { name: "Prompt" }), "Hi");
    await user.click(screen.getByRole("button", { name: "Send" }));
    const answer = await screen.findByRole("article", { name: "GPT-5.4 mini response" });
    await waitFor(() => expect(answer).toHaveTextContent("Partial answer"));
    await user.click(screen.getByRole("button", { name: "Stop" }));
    await waitFor(() => expect(answer).toHaveTextContent("Stopped"));
    expect(answer).toHaveTextContent("Partial answer");
    expect(await screen.findByRole("button", { name: "Send" })).toBeInTheDocument();
  });

  it("names the request of a gateway failure in the column", async () => {
    signedIn();
    on("GET", MODELS, page([playgroundModel()]));
    on("POST", RESPONSES, () => {
      const refused = apiError(502, "internal", "The gateway failed.");
      refused.headers.set("kaeso-request-id", "req_01abc");
      return refused;
    });
    const { user } = await renderApp(PLAYGROUND);

    await screen.findByText("gpt-5.4-mini");
    await user.type(screen.getByRole("textbox", { name: "Prompt" }), "Hi");
    await user.click(screen.getByRole("button", { name: "Send" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("The gateway failed. Request req_01abc");
  });

  it("says when the model cut a reply off at the output limit", async () => {
    signedIn();
    on("GET", MODELS, page([playgroundModel()]));
    on("POST", RESPONSES, () =>
      eventStream(
        sse([
          { type: "response.created" },
          { type: "response.output_text.delta", item_id: "m", delta: "The first half" },
          { type: "response.incomplete", response: { incomplete_details: { reason: "max_output_tokens" }, usage: { input_tokens: 3, output_tokens: 16 } } },
        ]),
      ),
    );
    const { user } = await renderApp(PLAYGROUND);

    await screen.findByText("gpt-5.4-mini");
    await user.type(screen.getByRole("textbox", { name: "Prompt" }), "Hi");
    await user.click(screen.getByRole("button", { name: "Send" }));
    const answer = await screen.findByRole("article", { name: "GPT-5.4 mini response" });
    await waitFor(() => expect(answer).toHaveTextContent("Stopped at the output limit"));
    expect(answer).toHaveTextContent("The first half");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("adds no column while the models haven't loaded", async () => {
    signedIn();
    on("GET", MODELS, apiError(403, "permission_denied", "Models are hidden from you."));
    const { user, router } = await renderApp(PLAYGROUND);

    expect((await notifications().findAllByText("Models are hidden from you.")).length).toBeGreaterThan(0);
    await user.click(screen.getByRole("button", { name: "Add column" }));
    expect(router.state.location.search).toEqual({});
    expect(screen.getAllByRole("button", { name: "Model" })).toHaveLength(1);
  });

  it("sends only to the columns that have a model", async () => {
    signedIn();
    on("GET", MODELS, page([playgroundModel()]));
    on("POST", RESPONSES, () => reply("Only one."));
    const { user } = await renderApp(`${PLAYGROUND}?models=openai/gone-model,openai/gpt-5.4-mini`);

    await screen.findByText("gpt-5.4-mini");
    expect(screen.getAllByRole("button", { name: "Model" })[0]).toHaveTextContent("Choose a model");
    await user.type(screen.getByRole("textbox", { name: "Prompt" }), "Hi");
    await user.click(screen.getByRole("button", { name: "Send" }));
    expect(await screen.findByText("Only one.")).toBeInTheDocument();
    expect(sentBodies()).toEqual([expect.objectContaining({ model: "gpt-5.4-mini" })]);
  });

  it("changes one column's model and keeps the others", async () => {
    signedIn();
    const nano = playgroundModel({ id: "cm_n", model: "gpt-5.4-nano", model_id: "gpt-5.4-nano", display_name: "gpt-5.4-nano" });
    on("GET", MODELS, page([playgroundModel(), claude, nano]));
    const { user, router } = await renderApp(`${PLAYGROUND}?models=openai/gpt-5.4-mini,anthropic/claude-opus-5-5`);

    await screen.findByText("claude-opus-5-5");
    await user.click(screen.getAllByRole("button", { name: "Model" })[1] as HTMLElement);
    const option = within(await screen.findByRole("listbox")).getByRole("option", { name: /gpt-5\.4-nano/ });
    expect(within(option).getAllByText("gpt-5.4-nano")).toHaveLength(1);
    await user.click(option);
    await waitFor(() => expect(router.state.location.search).toEqual({ models: "openai/gpt-5.4-mini,openai/gpt-5.4-nano" }));
  });
});
