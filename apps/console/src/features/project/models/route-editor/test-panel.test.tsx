import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { RouteStep } from "~/api/generated/types";
import { USER, signedIn, without } from "~/test/fixtures";
import { eventStream, sse } from "~/test/fixtures/project-logs";
import { alias, apiKey, catalogueModel, modelRoute, modelsApi, projectData, projectPath } from "~/test/fixtures/project";
import { on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const ONE_ALIAS = "orgs/:org/projects/:project/aliases/:alias";
const TRACE = "orgs/:org/projects/:project/aliases/trace";
const RESPONSES = "orgs/:org/projects/:project/playground/responses";
const ROUTE = "kaeso-route";

type Sent = {
  request: {
    headers: { name: string; value: string }[];
    format: string;
    features: string[];
    input_tokens: number;
    key: string | null;
    service_account: string | null;
  };
};

const models = () => [
  catalogueModel(),
  catalogueModel({ id: "cm_haiku", model_id: "claude-haiku-4-5", effective: { ...catalogueModel().effective, display_name: "Claude Haiku 4.5" } }),
];

const route: RouteStep = {
  type: "condition",
  match: "any",
  rules: [
    { field: "header", name: "x-plan", op: "equals", value: "enterprise" },
    { field: "input_tokens", op: "at_least", value: 100000 },
  ],
  then: modelRoute("cm_sonnet"),
  else: {
    type: "split",
    by: "header",
    header: "x-user-id",
    branches: [
      { weight: 30, step: modelRoute("cm_sonnet") },
      { weight: 70, step: modelRoute("cm_haiku") },
    ],
  },
};

const last = () => requests("POST", TRACE).at(-1)?.body as Sent;

async function open() {
  signedIn();
  const one = alias({ route });
  projectData({
    aliases: [one],
    models: models(),
    keys: [apiKey({ id: "key_batch", name: "nightly-batch", principal: { kind: "service_account", id: "sa_batch" } })],
  });
  modelsApi({ models: models() });
  on("GET", ONE_ALIAS, one);
  on("POST", TRACE, ({ body }: { body: Sent }) =>
    body.request.input_tokens >= 100000
      ? {
          steps: [
            { type: "condition", path: "route", holds: true, rules: [false, true] },
            { type: "model", path: "route.then", catalogue_model_id: "cm_sonnet", attempt: 1 },
          ],
        }
      : {
          steps: [
            { type: "condition", path: "route", holds: false, rules: [false, false] },
            { type: "split", path: "route.else", branch: 0, weight: 30, draw: 12 },
            { type: "model", path: "route.else.branches.0.step", catalogue_model_id: "cm_sonnet", attempt: 1 },
          ],
        },
  );
  const app = await renderApp(projectPath("models/aliases/al_fast"));
  await app.user.click(await screen.findByRole("tab", { name: "Test" }));
  await app.user.click(await screen.findByRole("radio", { name: "Preview" }));
  return { ...app, panel: await screen.findByRole("complementary", { name: "Step settings" }) };
}

describe("TestPanel", () => {
  it("offers every header the route reads and sends each fact of the request", async () => {
    const { user, panel } = await open();

    expect(within(panel).getByRole("textbox", { name: "Name of header 1" })).toHaveValue("x-plan");
    expect(within(panel).getByRole("textbox", { name: "Name of header 2" })).toHaveValue("x-user-id");
    expect(await within(panel).findByText("0 of 2 rules hold; takes Else")).toBeInTheDocument();
    expect(within(panel).getByText("Drew 12 of 100; takes branch 1, 30%")).toBeInTheDocument();

    await user.click(within(panel).getByRole("combobox", { name: "API format" }));
    await user.click(await screen.findByRole("option", { name: "Anthropic Messages" }));
    await waitFor(() => expect(last().request.format).toBe("anthropic_messages"));

    await user.click(within(panel).getByRole("combobox", { name: "Uses" }));
    await user.click(await screen.findByRole("option", { name: "Images" }));
    await user.keyboard("{Escape}");
    await waitFor(() => expect(last().request.features).toEqual(["images"]));

    await user.click(within(panel).getByRole("combobox", { name: "API key" }));
    await user.click(await screen.findByRole("option", { name: /nightly-batch/ }));
    await waitFor(() => expect(last().request).toMatchObject({ key: "key_batch", service_account: "sa_batch" }));
    await user.click(within(panel).getByRole("combobox", { name: "API key" }));
    await user.click(await screen.findByRole("option", { name: "No key" }));
    expect(within(panel).getByRole("combobox", { name: "API key" })).toHaveTextContent("No key");

    const tokens = within(panel).getByRole("textbox", { name: "Input tokens" });
    await user.clear(tokens);
    expect(tokens).toHaveValue("");
    await user.type(tokens, "150k000");
    expect(tokens).toHaveValue("150000");
    expect(await within(panel).findByText("1 of 2 rules hold; takes Then")).toBeInTheDocument();
    expect(last().request.input_tokens).toBe(150000);
  });

  it("stays on the trace when the background is clicked, and opens the alias when the start is", async () => {
    const { user, panel } = await open();
    await within(panel).findByText("0 of 2 rules hold; takes Else");

    fireEvent.pointerDown(document.querySelector("[data-flow-canvas]") as HTMLElement, { button: 0, pointerId: 1, pointerType: "mouse" });
    fireEvent.pointerUp(document.querySelector("[data-flow-canvas]") as HTMLElement, { button: 0, pointerId: 1, pointerType: "mouse" });
    expect(screen.getByRole("tab", { name: "Test" })).toHaveAttribute("aria-selected", "true");

    await user.click(screen.getByRole("treeitem", { name: /^Start/ }));
    expect(await screen.findByRole("heading", { name: "Alias" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Editor" })).toHaveAttribute("aria-selected", "true");
  });
});

const chain: RouteStep = modelRoute("cm_sonnet", modelRoute("cm_haiku"));
const lastTrace = () => requests("POST", TRACE).at(-1)?.body as Sent & { request: { member: string | null } };
const sentBody = () => requests("POST", RESPONSES).at(-1)?.body as Record<string, unknown>;
const reply = (
  text: string,
  headers: Record<string, string>,
  end: { type: string; [field: string]: unknown } = { type: "response.completed", response: { usage: { input_tokens: 9, output_tokens: 4 } } },
) =>
  eventStream(
    sse([
      { type: "response.created" },
      { type: "response.reasoning_summary_text.delta", item_id: "r", summary_index: 0, delta: "Weighing it." },
      { type: "response.output_text.delta", item_id: "m", delta: text },
      { type: "response.output_text.done", item_id: "m" },
      end,
    ]),
    headers,
  );

async function openSend({ rights, route = chain, path = "models/aliases/al_fast" }: { rights?: string[]; route?: RouteStep; path?: string } = {}) {
  signedIn(rights ? { projectRights: rights } : {});
  const one = alias({ route });
  projectData({ aliases: [one], models: models(), keys: [] });
  modelsApi({ models: models() });
  on("GET", ONE_ALIAS, one);
  on("POST", TRACE, {
    steps: [
      { type: "model", path: "route", catalogue_model_id: "cm_sonnet", attempt: 1 },
      { type: "model", path: "route.fallback", catalogue_model_id: "cm_haiku", attempt: 2 },
    ],
  });
  const app = await renderApp(projectPath(path));
  await app.user.click(await screen.findByRole("tab", { name: "Test" }));
  return { ...app, panel: await screen.findByRole("complementary", { name: "Step settings" }) };
}

const node = (name: RegExp) => {
  const [found] = screen.getAllByRole("treeitem", { name }).filter((i) => !/^(Add|Insert)/.test(i.getAttribute("aria-label") ?? i.textContent ?? ""));
  if (!found) throw new Error(`no step ${name}`);
  return found;
};

describe("Sending a request", () => {
  it("sends the draft through the route as the member and colors each step by its answer", async () => {
    const { user, panel, router } = await openSend();
    on(
      "POST",
      RESPONSES,
      reply("Hello there.", {
        [ROUTE]: '"route";model="anthropic/claude-sonnet-4-5";status=529;dur=1204, "route.fallback";model="anthropic/claude-haiku-4-5";status=200',
        "kaeso-request-id": "req_42",
      }),
    );
    await waitFor(() => expect(lastTrace().request).toMatchObject({ format: "openai_responses", features: ["streaming"], member: USER, key: null }));
    expect(node(/Claude Sonnet 4\.5/)).toHaveTextContent("Try 1");
    const message = within(panel).getByRole("textbox", { name: "Message" });
    expect(message).toHaveValue("Say hello in one sentence.");

    await user.click(within(panel).getByRole("button", { name: "Send" }));
    expect(await within(panel).findByText("Hello there.")).toBeInTheDocument();
    expect(sentBody()).toMatchObject({
      model: "fast",
      input: "Say hello in one sentence.",
      stream: true,
      alias: "al_fast",
      headers: [],
      route: { type: "model", model: "cm_sonnet", fallback: { type: "model", model: "cm_haiku" } },
    });
    expect(message).toHaveValue("Say hello in one sentence.");
    expect(within(panel).getByText("Failed after 1.2 s; fell back")).toBeInTheDocument();
    expect(within(panel).getByText("Answered")).toBeInTheDocument();
    expect(within(panel).getByText("529")).toBeInTheDocument();
    expect(node(/Claude Sonnet 4\.5/)).toHaveTextContent("529 after 1.2 s");
    expect(node(/Claude Sonnet 4\.5/)).toHaveClass("ring-invalid");
    expect(node(/Claude Haiku 4\.5/)).toHaveClass("ring-success");
    expect(document.querySelector("[data-tone='success']")).not.toBeNull();

    await user.click(within(panel).getByRole("button", { name: "Open in request log" }));
    await waitFor(() => expect(router.state.location.search).toMatchObject({ log: "req_42" }));
  });

  it("shows a route whose every step failed, with skipped steps and why", async () => {
    const { user, panel } = await openSend({
      route: {
        type: "condition",
        match: "all",
        rules: [{ field: "header", name: "x-plan", op: "equals", value: "paid" }],
        then: chain,
        else: modelRoute("cm_haiku"),
      },
    });
    on(
      "POST",
      RESPONSES,
      new Response(
        JSON.stringify({ error: { type: "provider_error", message: "The provider is overloaded right now; try again shortly.", param: null, right: null } }),
        {
          status: 503,
          headers: {
            "content-type": "application/json",
            [ROUTE]: '"route.then";skipped, "route.then.fallback";model="anthropic/claude-haiku-4-5";status=529;dur=40',
            "kaeso-request-id": "req_43",
          },
        },
      ),
    );
    await user.type(within(panel).getByRole("textbox", { name: "Value of header 1" }), "paid");
    await user.click(within(panel).getByRole("button", { name: "Send" }));
    expect(await within(panel).findByText("The provider is overloaded right now; try again shortly.")).toBeInTheDocument();
    expect(sentBody()).toMatchObject({ headers: [{ name: "x-plan", value: "paid" }] });
    expect(within(panel).getByText("Takes Then")).toBeInTheDocument();
    expect(within(panel).getByText("Skipped; you may not use this model")).toBeInTheDocument();
    expect(within(panel).getByText("Failed after 40 ms")).toBeInTheDocument();
    const [skipped, failed] = screen
      .getAllByRole("treeitem")
      .filter((i) => i.getAttribute("data-trace") === "on")
      .slice(2);
    expect(skipped).toHaveTextContent("Skipped");
    expect(failed).toHaveClass("ring-invalid");
  });

  it("tells the branch a split took, and that a step failed without a time", async () => {
    const { user, panel } = await openSend({
      rights: without("logs.metadata.view"),
      route: {
        type: "condition",
        match: "all",
        rules: [{ field: "header", name: "x-plan", op: "equals", value: "paid" }],
        then: modelRoute("cm_haiku"),
        else: {
          type: "split",
          by: "request",
          header: null,
          branches: [
            { weight: 40, step: modelRoute("cm_sonnet") },
            { weight: 60, step: chain },
          ],
        },
      },
    });
    on(
      "POST",
      RESPONSES,
      reply(
        "Hi.",
        {
          [ROUTE]: '"route.else.branches.1.step";model="anthropic/claude-sonnet-4-5";status=500, "route.else.branches.1.step.fallback";status=429',
          "kaeso-request-id": "req_44",
        },
        { type: "response.incomplete", response: { incomplete_details: { reason: "max_output_tokens" } } },
      ),
    );
    await user.click(within(panel).getByRole("button", { name: "Send" }));
    expect(await within(panel).findByText("Takes Else")).toBeInTheDocument();
    expect(within(panel).getByText("Takes branch 2, 60%; the next request is drawn anew.")).toBeInTheDocument();
    expect(within(panel).getByText("Failed; fell back")).toBeInTheDocument();
    expect(within(panel).getByText("Failed")).toBeInTheDocument();
    expect(within(panel).queryByRole("button", { name: "Open in request log" })).not.toBeInTheDocument();
  });

  it("says a split by caller keeps all your requests on one branch, and a split by header keeps a value on one", async () => {
    const split = (by: "caller" | "header"): RouteStep => ({
      type: "split",
      by,
      header: by === "header" ? "x-user-id" : null,
      branches: [
        { weight: 80, step: modelRoute("cm_haiku") },
        { weight: 20, step: modelRoute("cm_sonnet") },
      ],
    });
    const { user, panel } = await openSend({ route: split("caller") });
    expect(within(panel).getByText("A split by caller sends all your requests down one branch; Preview shows where a key's go.")).toBeInTheDocument();
    on("POST", RESPONSES, reply("Hi.", { [ROUTE]: '"route.branches.1.step";model="anthropic/claude-sonnet-4-5";status=200' }));
    await user.click(within(panel).getByRole("button", { name: "Send" }));
    expect(await within(panel).findByText("Takes branch 2, 20%, like all your requests: each caller stays on one branch.")).toBeInTheDocument();
    await user.click(within(panel).getByRole("radio", { name: "Preview" }));
    expect(await within(panel).findByText("A split by caller keeps each key's requests on one branch.")).toBeInTheDocument();
    expect(within(panel).getByRole("button", { name: "Draw again" })).toBeInTheDocument();
  });

  it("says a header's value keeps a split by header on one branch once the header is sent", async () => {
    const { user, panel } = await openSend({
      route: {
        type: "split",
        by: "header",
        header: "x-user-id",
        branches: [
          { weight: 80, step: modelRoute("cm_haiku") },
          { weight: 20, step: modelRoute("cm_sonnet") },
        ],
      },
    });
    expect(within(panel).queryByText(/A split by caller/)).not.toBeInTheDocument();
    await user.type(within(panel).getByRole("textbox", { name: "Value of header 1" }), "user-7");
    on("POST", RESPONSES, reply("Hi.", { [ROUTE]: '"route.branches.0.step";model="anthropic/claude-haiku-4-5";status=200' }));
    await user.click(within(panel).getByRole("button", { name: "Send" }));
    expect(await within(panel).findByText("Takes branch 1, 80%, like every request with this x-user-id.")).toBeInTheDocument();
    await user.click(within(panel).getByRole("radio", { name: "Preview" }));
    expect(await within(panel).findByRole("combobox", { name: "API format" })).toBeInTheDocument();
    expect(within(panel).queryByRole("button", { name: "Draw again" })).not.toBeInTheDocument();
    await user.clear(within(panel).getByRole("textbox", { name: "Value of header 1" }));
    expect(await within(panel).findByRole("button", { name: "Draw again" })).toBeInTheDocument();
  });

  it("stops a request, says when a stream ends early, and forgets the colors once the route changes", async () => {
    const { user, panel } = await openSend();
    on(
      "POST",
      RESPONSES,
      () =>
        new Response(
          new ReadableStream({
            start(controller) {
              controller.enqueue(new TextEncoder().encode('event: response.created\ndata: {"type":"response.created"}\n\n'));
            },
          }),
          { status: 200, headers: { "content-type": "text/event-stream", [ROUTE]: '"route";model="anthropic/claude-sonnet-4-5";status=200' } },
        ),
    );
    await user.click(within(panel).getByRole("button", { name: "Send" }));
    await user.click(await within(panel).findByRole("button", { name: "Stop" }));
    expect(await within(panel).findByRole("button", { name: "Send" })).toBeInTheDocument();

    on(
      "POST",
      RESPONSES,
      eventStream('data: not json\n\ndata: 5\n\nevent: response.created\ndata: {"type":"response.created"}\n\n', {
        [ROUTE]: '"route";model="anthropic/claude-sonnet-4-5";status=200',
      }),
    );
    await user.click(within(panel).getByRole("button", { name: "Send" }));
    expect(await within(panel).findByText("The stream ended before the model finished. Try again.")).toBeInTheDocument();
    expect(node(/Claude Sonnet 4\.5/)).toHaveClass("ring-success");

    await user.click(screen.getByRole("tab", { name: "Editor" }));
    await user.click(await screen.findByRole("treeitem", { name: "Add a fallback to Claude Haiku 4.5" }));
    await user.click(await screen.findByRole("menuitem", { name: "Fall back to a model" }));
    await user.click(screen.getByRole("tab", { name: "Test" }));
    expect(await within(panel).findByText("Fix the steps marked on the route first.")).toBeInTheDocument();
    expect(node(/Claude Sonnet 4\.5/)).not.toHaveClass("ring-success");
  });

  it("says why it can't send without the Playground right, and switches between sending and previewing", async () => {
    const { user, panel } = await openSend({ rights: without("playground.use") });
    expect(within(panel).getByText(/Use the Playground/)).toBeInTheDocument();
    expect(within(panel).getByRole("textbox", { name: "Message" })).toBeDisabled();
    await user.click(within(panel).getByRole("radio", { name: "Preview" }));
    expect(await within(panel).findByRole("combobox", { name: "API format" })).toBeInTheDocument();
    await user.click(within(panel).getByRole("radio", { name: "Real request" }));
    expect(within(panel).getByRole("textbox", { name: "Message" })).toBeInTheDocument();
  });

  it("says when the connection breaks off, and sends a new alias without an id", async () => {
    const { user, panel } = await openSend({ path: "models/aliases/new" });
    await user.click(screen.getByRole("tab", { name: "Editor" }));
    await user.type(await screen.findByRole("textbox", { name: "Name" }), "fresh");
    await user.click(screen.getByRole("combobox", { name: "Start from" }));
    await user.click(await screen.findByRole("option", { name: /An A\/B test/ }));
    await user.click(screen.getByRole("tab", { name: "Test" }));
    let answer: (r: Response) => void = () => undefined;
    on("POST", RESPONSES, () => new Promise<Response>((resolve) => (answer = resolve)));
    await user.click(await within(panel).findByRole("button", { name: "Send" }));
    expect(await within(panel).findByText("On its way along the route")).toBeInTheDocument();
    expect(document.querySelector("[data-flowing]")).not.toBeNull();
    answer(
      new Response(
        new ReadableStream({
          start(controller) {
            controller.enqueue(new TextEncoder().encode('event: response.created\ndata: {"type":"response.created"}\n\n'));
            controller.error(new TypeError("network"));
          },
        }),
        { status: 200, headers: { "content-type": "text/event-stream" } },
      ),
    );
    expect(await within(panel).findByText("The connection closed before the reply finished. Try again.")).toBeInTheDocument();
    expect(within(panel).queryByText("Steps it took")).not.toBeInTheDocument();
    expect(sentBody()).toMatchObject({ model: "fresh" });
    expect(sentBody()).not.toHaveProperty("alias");
  });

  it("asks for a name before it sends", async () => {
    const { user } = await openSend();
    await user.click(screen.getByRole("tab", { name: "Editor" }));
    await user.clear(await screen.findByRole("textbox", { name: "Name" }));
    await user.click(screen.getByRole("tab", { name: "Test" }));
    expect(await screen.findByText("Give the alias a name first.")).toBeInTheDocument();
  });
});
