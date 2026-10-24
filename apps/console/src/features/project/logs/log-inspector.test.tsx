import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { signedIn, without } from "~/test/fixtures";
import { apiKey, catalogueModel, projectData, projectPath, serviceAccount } from "~/test/fixtures/project";
import { logContent, logEntry, logRow, logsData } from "~/test/fixtures/project-logs";
import { apiError, on } from "~/test/network";
import { renderApp } from "~/test/render";

const sheet = () => screen.findByRole("dialog", { name: /^\d{3}/ });
const section = (container: HTMLElement, name: string) => within(container).getByRole("heading", { name }).closest("section") as HTMLElement;
const noSection = (container: HTMLElement, name: string) => expect(within(container).queryByRole("heading", { name })).not.toBeInTheDocument();
const openTab = async (user: { click: (el: Element) => Promise<void> }, name: string) => {
  await user.click(within(await sheet()).getByRole("tab", { name }));
  return within(await sheet()).findByRole("tabpanel");
};

const chatRequest = {
  model: "gpt-5.4-mini",
  temperature: 0.2,
  max_tokens: 300,
  messages: [
    { role: "system", content: "Be brief." },
    { role: "user", content: "Where is order 7?" },
  ],
  tools: [
    { type: "function", function: { name: "lookup_order", description: "Find an order by number", parameters: { type: "object" } } },
    { type: "web_search" },
  ],
};
const kppReply = {
  blocks: [{ text: { text: "Order 7 shipped yesterday." } }, { unknown: { kind: "citation", url: "https://example.com" } }],
  id: "resp_1",
  model: "gpt-5.4-mini",
  stop: "end_turn",
};

describe("LogInspector", () => {
  it("sums up a failed streamed request from the playground", async () => {
    signedIn();
    projectData({ accounts: [serviceAccount()], models: [catalogueModel()] });
    logsData({
      rows: [logRow()],
      entries: [
        logEntry({
          id: "req_0042",
          status: 502,
          endpoint: "/v1/messages",
          provider: "anthropic",
          model: "claude-sonnet-4-5",
          requested_model: "fast",
          ingress_dialect: "openai_chat",
          egress_dialect: "anthropic_messages",
          error_kind: "stream_interrupted",
          error_message: "The provider closed the stream.",
          stream: true,
          stream_progress: { events: 12, bytes: 2048, first_byte_ms: null, keepalives: 3, processing_ms: 0 },
          ttfb_ms: null,
          overhead_ms: 14,
          key_id: null,
          has_content: false,
          upstream_request_id: "",
          user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15",
          principal_kind: "service_account",
          principal_id: "sa_deploy",
          usage: { input_tokens: 100, output_tokens: 40, cache_read_tokens: 20, cache_write_tokens: 0, reasoning_tokens: 10, estimated: true },
        }),
      ],
    });
    await renderApp(projectPath("logs?log=req_0042"));

    const dialog = await sheet();
    expect(dialog).toHaveAccessibleName(/^502.*Claude Sonnet 4\.5$/);
    expect(dialog).toHaveTextContent("/v1/messages · req_0042");
    expect(within(dialog).getByText("Streamed")).toBeInTheDocument();
    expect(dialog).toHaveTextContent("Content not logged: logging was off for the organization or the project, or the content has passed its retention.");
    const outcome = section(dialog, "Outcome");
    expect(outcome).toHaveTextContent("Stream interrupted");
    expect(outcome).toHaveTextContent("Message from AnthropicThe provider closed the stream.");
    expect(within(dialog).getByRole("list", { name: "Summary" })).toHaveTextContent("None arrived");
    expect(dialog).toHaveTextContent("120 in · 30 out · 10 reasoning");
    const timing = section(dialog, "Timing");
    expect(within(timing).queryByRole("img")).not.toBeInTheDocument();
    expect(timing).toHaveTextContent("Kaeso overhead14 ms");
    expect(timing).toHaveTextContent("Stream processingUnder 1 ms");
    const route = section(dialog, "Route");
    expect(route).toHaveTextContent("OpenAI Chat Completions → Anthropic Messages");
    expect(route).toHaveTextContent("Model, requested as fast");
    expect(route).toHaveTextContent("Claude Sonnet 4.5claude-sonnet-4-5");
    expect(route).toHaveTextContent("Anthropic");
    const sentBy = section(dialog, "Sent by");
    expect(sentBy).toHaveTextContent("Service account");
    expect(sentBy).toHaveTextContent("ci-deploy");
    expect(sentBy).toHaveTextContent("Playground");
    expect(sentBy).toHaveTextContent("Safari on macOS");
    const stream = section(dialog, "Stream");
    expect(stream).toHaveTextContent("12");
    expect(stream).toHaveTextContent("2.0 KB");
    expect(stream).toHaveTextContent("None");
    const usage = section(dialog, "Usage");
    expect(usage).toHaveTextContent("Estimated");
    expect(usage).toHaveTextContent("The stream ended before the provider's final count; cost and limits use these numbers.");
  });

  it("names a replaced key as replaced", async () => {
    signedIn();
    projectData({ keys: [apiKey({ replaced_by: "key_next" })] });
    logsData({ rows: [logRow()], entries: [logEntry()] });
    await renderApp(projectPath("logs?log=req_0001"));

    expect(section(await sheet(), "Sent by")).toHaveTextContent("checkout-web (replaced)");
  });

  it("shows the timing and the key of a plain request", async () => {
    signedIn();
    projectData({ keys: [apiKey()] });
    logsData({
      rows: [logRow()],
      entries: [logEntry({ usage: { input_tokens: 0, output_tokens: 0, cache_read_tokens: 0, cache_write_tokens: 0, reasoning_tokens: 0, estimated: false } })],
    });
    await renderApp(projectPath("logs?log=req_0001"));

    const dialog = await sheet();
    expect(dialog).toHaveTextContent("/v1/chat/completions · req_0001");
    const timing = section(dialog, "Timing");
    expect(within(timing).getByRole("img")).toHaveAccessibleName("Time to first byte 420 ms of 1.3 s total");
    expect(timing).toHaveTextContent("Kaeso overhead3 ms");
    expect(timing).not.toHaveTextContent("Stream processing");
    expect(within(dialog).getByRole("list", { name: "Summary" })).toHaveTextContent("From the provider");
    expect(within(dialog).getByRole("list", { name: "Summary" })).toHaveTextContent("None counted");
    noSection(dialog, "Outcome");
    noSection(dialog, "Usage");
    noSection(dialog, "Stream");
    const route = section(dialog, "Route");
    expect(route).toHaveTextContent("OpenAI Chat Completions");
    expect(route).not.toHaveTextContent("→");
    expect(route).toHaveTextContent("chatcmpl-abc123");
    const sentBy = section(dialog, "Sent by");
    expect(sentBy).toHaveTextContent("Member");
    expect(sentBy).toHaveTextContent("Ada Okafor (you)");
    expect(sentBy).toHaveTextContent("checkout-web");
    expect(sentBy).toHaveTextContent("openai-python/1.40.0");
  });

  it("shows dashes for a request that never reached a model, and the stream's first byte", async () => {
    signedIn();
    projectData();
    logsData({
      rows: [logRow()],
      entries: [
        logEntry({
          status: 404,
          model: "",
          provider: "",
          requested_model: "",
          error_kind: "model_not_found",
          error_message: null,
          upstream_request_id: "",
          user_agent: "",
          stream: true,
          stream_progress: { events: 1, bytes: 300, first_byte_ms: 320, keepalives: 0 },
          ttfb_ms: null,
          overhead_ms: null,
        }),
      ],
    });
    await renderApp(projectPath("logs?log=req_0001"));

    const dialog = await sheet();
    expect(dialog).toHaveAccessibleName(/\/v1\/chat\/completions$/);
    const route = section(dialog, "Route");
    expect(route).toHaveTextContent("Model—Provider—Upstream request—");
    const sentBy = section(dialog, "Sent by");
    expect(sentBy).toHaveTextContent("User agent—");
    expect(within(sentBy).queryAllByRole("button")).toHaveLength(0);
    expect(within(route).queryAllByRole("button")).toHaveLength(0);
    expect(section(dialog, "Stream")).toHaveTextContent("320 ms");
    noSection(dialog, "Timing");
  });

  it("says when a request is gone and goes back to the list", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()] });
    const { user, router } = await renderApp(projectPath("logs?log=req_gone"));

    const dialog = await screen.findByRole("dialog", { name: "Request" });
    expect(await within(dialog).findByText("This request doesn't exist or was removed.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Back to requests" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("says when a request couldn't load", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()] });
    on("GET", "orgs/:org/projects/:project/logs/:log", apiError(403, "permission_denied", "You can't open this request."));
    await renderApp(projectPath("logs?log=req_0001"));

    const dialog = await screen.findByRole("dialog", { name: "Request" });
    expect(await within(dialog).findByText("Couldn't load this request.")).toBeInTheDocument();
  });

  it("draws the request as a conversation with its tools and parameters", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()], entries: [logEntry({ content: logContent(chatRequest, kppReply) })] });
    const { user } = await renderApp(projectPath("logs?log=req_0001"));

    const panel = await openTab(user, "Request");
    expect(panel).toHaveTextContent("The app's body as sent, in OpenAI Chat Completions.");
    expect(await within(panel).findByRole("region", { name: "System" })).toHaveTextContent("Be brief.");
    expect(within(panel).getByRole("region", { name: "User" })).toHaveTextContent("Where is order 7?");
    const params = section(panel, "Parameters");
    expect(params).toHaveTextContent("Max tokens");
    expect(params).toHaveTextContent("300");
    expect(params).toHaveTextContent("Temperature");
    await user.click(within(panel).getByRole("button", { name: /Tools/ }));
    expect(await within(panel).findByText("lookup_order")).toBeInTheDocument();
    expect(within(panel).getByText("Find an order by number")).toBeInTheDocument();
    expect(within(panel).getByText("web_search")).toBeInTheDocument();

    await user.click(within(panel).getByRole("switch", { name: "Show raw" }));
    await waitFor(() => expect(within(panel).queryByRole("region", { name: "System" })).not.toBeInTheDocument());
    expect(panel).toHaveTextContent("messages");
  });

  it("shows a body it can't map as JSON", async () => {
    signedIn();
    projectData();
    logsData({
      rows: [logRow()],
      entries: [logEntry({ ingress_dialect: "custom", egress_dialect: "custom", content: logContent({ prompt: "Hello there" }, { text: "Hi" }) })],
    });
    const { user } = await renderApp(projectPath("logs?log=req_0001"));

    const request = await openTab(user, "Request");
    expect(request).toHaveTextContent("prompt");
    expect(within(request).queryByRole("switch", { name: "Show raw" })).not.toBeInTheDocument();
    const reply = await openTab(user, "Reply");
    expect(reply).toHaveTextContent("The reply in Kaeso's own format, which also keeps what the app's format can't show.");
    expect(within(reply).queryByRole("switch", { name: "Show raw" })).not.toBeInTheDocument();
  });

  it("counts embedded inputs and explains that vectors aren't kept", async () => {
    signedIn();
    projectData();
    logsData({
      rows: [logRow()],
      entries: [
        logEntry({
          endpoint: "/v1/embeddings",
          ingress_dialect: "openai_embeddings",
          egress_dialect: "openai_embeddings",
          usage: { input_tokens: 8, output_tokens: 0, cache_read_tokens: 0, cache_write_tokens: 0, reasoning_tokens: 0, estimated: false },
          content: logContent({ model: "text-embedding-3-small", input: ["one", "two"] }, null),
        }),
      ],
    });
    const { user } = await renderApp(projectPath("logs?log=req_0001"));

    const usage = section(await sheet(), "Usage");
    expect(within(usage).getByText("Input")).toBeInTheDocument();
    expect(within(usage).queryByText("Cache read")).toBeNull();
    expect(within(usage).queryByText("Reasoning")).toBeNull();

    const request = await openTab(user, "Request");
    expect(request).toHaveTextContent("2 inputs embedded.");
    const reply = await openTab(user, "Reply");
    expect(reply).toHaveTextContent("Vectors aren't kept");
  });

  it("draws the reply with how it stopped and the blocks kept as they came", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()], entries: [logEntry({ stream: true, content: logContent(chatRequest, kppReply) })] });
    const { user } = await renderApp(projectPath("logs?log=req_0001"));

    const reply = await openTab(user, "Reply");
    expect(reply).toHaveTextContent("Assembled from the stream in Kaeso's own format, which also keeps what the app's format can't show.");
    expect(within(reply).getByText("End of turn")).toBeInTheDocument();
    expect(await within(reply).findByRole("region", { name: "Assistant" })).toHaveTextContent("Order 7 shipped yesterday.");
    expect(reply).toHaveTextContent("A block kept as it came:");
    await user.click(within(reply).getByRole("switch", { name: "Show raw" }));
    await waitFor(() => expect(within(reply).queryByRole("region", { name: "Assistant" })).not.toBeInTheDocument());
    expect(reply).toHaveTextContent("blocks");
  });

  it("shows a reply logged in a newer format as stored, with a note", async () => {
    signedIn();
    projectData();
    const newer = { format: 2, parts: [{ kind: "text", value: "Order 7 shipped yesterday." }], stop: "end_turn" };
    logsData({ rows: [logRow()], entries: [logEntry({ content: logContent(chatRequest, newer) })] });
    const { user } = await renderApp(projectPath("logs?log=req_0001"));

    const reply = await openTab(user, "Reply");
    expect(reply).toHaveTextContent("This reply was logged in a format this console can't read yet; it's shown as stored.");
    expect(reply).toHaveTextContent("parts");
    expect(within(reply).queryByRole("region", { name: "Assistant" })).not.toBeInTheDocument();
    expect(within(reply).queryByRole("switch", { name: "Show raw" })).not.toBeInTheDocument();
    expect(within(reply).queryByText("End of turn")).not.toBeInTheDocument();
  });

  it("marks the reply of a stream that stopped as incomplete", async () => {
    signedIn();
    projectData();
    logsData({
      rows: [logRow()],
      entries: [logEntry({ stream: true, status: 502, error_kind: "stream_interrupted", content: logContent(chatRequest, kppReply) })],
    });
    const { user } = await renderApp(projectPath("logs?log=req_0001"));

    const reply = await openTab(user, "Reply");
    expect(within(reply).getByText("Incomplete")).toBeInTheDocument();
    expect(reply).toHaveTextContent("Assembled from the stream until it stopped, in Kaeso's own format, which also keeps what the app's format can't show.");
  });

  it("explains a missing reply", async () => {
    signedIn();
    projectData();
    logsData({
      rows: [logRow()],
      entries: [
        logEntry({ id: "req_0001", status: 429, error_kind: "rate_limited", content: logContent(chatRequest, null) }),
        logEntry({ id: "req_0002", content: logContent(chatRequest, null) }),
      ],
    });
    const { user, router } = await renderApp(projectPath("logs?log=req_0001"));

    let reply = await openTab(user, "Reply");
    expect(reply).toHaveTextContent("No reply");
    expect(reply).toHaveTextContent("The request failed before a reply arrived.");

    await router.navigate({ to: "/orgs/$org/projects/$project/logs", params: { org: "org_northwind", project: "proj_checkout" }, search: { log: "req_0002" } });
    reply = await openTab(user, "Reply");
    expect(reply).toHaveTextContent("No reply was stored for this request.");
  });

  it("says when content wasn't logged", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()], entries: [logEntry({ has_content: false, content: null })] });
    const { user } = await renderApp(projectPath("logs?log=req_0001"));

    const request = await openTab(user, "Request");
    expect(request).toHaveTextContent("Content not logged");
    expect(request).toHaveTextContent("Logging was off when this request ran, or its content has passed the retention period.");
    expect(await openTab(user, "Reply")).toHaveTextContent("Content not logged");
  });

  it("remembers a request refused before a model was chosen by its status and endpoint", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()], entries: [logEntry({ status: 401, model: "", provider: "" })] });
    await renderApp(projectPath("logs?log=req_0001"));

    await sheet();
    await waitFor(() =>
      expect(JSON.parse(window.localStorage.getItem("kaeso:recent") ?? "[]")).toContainEqual(
        expect.objectContaining({ kind: "request", label: "401 /v1/chat/completions" }),
      ),
    );
  });

  it("hides the content without the right to see it", async () => {
    signedIn({ projectRights: without("logs.content.view") });
    projectData();
    logsData({ rows: [logRow()], entries: [logEntry({ content: logContent(chatRequest, kppReply) })] });
    const { user } = await renderApp(projectPath("logs?log=req_0001"));

    const request = await openTab(user, "Request");
    expect(request).toHaveTextContent("You can't see request content");
    expect(await within(request).findByText("You need View prompts and outputs; ask Lin Wei for it.")).toBeInTheDocument();
    expect(await openTab(user, "Reply")).toHaveTextContent("You can't see request content");
    const raw = await openTab(user, "Raw");
    expect(raw).toHaveTextContent("Metadata");
    expect(raw).not.toHaveTextContent("Order 7 shipped yesterday.");
  });

  it("shows the raw entry with its content", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()], entries: [logEntry({ content: logContent(chatRequest, kppReply) })] });
    const { user } = await renderApp(projectPath("logs?log=req_0001"));

    const raw = await openTab(user, "Raw");
    expect(raw).toHaveTextContent("The log entry as the API returns it.");
    expect(within(raw).getByRole("button", { name: "req_0001" })).toBeInTheDocument();
    expect(raw).toHaveTextContent("Metadata");
    expect(raw).toHaveTextContent("Order 7 shipped yesterday.");
    expect(raw).toHaveTextContent("Where is order 7?");
  });

  it("says an outcome once and shows the gateway's message in the person's language with its code", async () => {
    signedIn();
    projectData();
    logsData({
      rows: [logRow()],
      entries: [
        logEntry({
          status: 404,
          provider: "groq",
          model: "llama-3.1-8b-instant",
          error_kind: "not_found",
          error_message:
            "Groq does not serve the model `llama-3.1-8b-instant` to this project's provider key; check the key's access to it, or remove the model from the catalogue.",
        }),
      ],
    });
    await renderApp(projectPath("logs?log=req_0001"), { locale: "de" });

    const outcome = section(await sheet(), "Ergebnis");
    expect(outcome).toHaveTextContent("404Nicht gefunden");
    expect(within(outcome).queryByText("Fehler")).toBeNull();
    expect(within(outcome).getByText("Meldung")).toBeInTheDocument();
    expect(await within(outcome).findByText("llama-3.1-8b-instant", { selector: "code" })).toBeInTheDocument();
    expect(outcome).toHaveTextContent("Groq bietet das Modell llama-3.1-8b-instant dem Anbieter-Schlüssel dieses Projekts nicht an;");
  });

  it("names the provider whose own words a message is", async () => {
    signedIn();
    projectData();
    logsData({
      rows: [logRow()],
      entries: [logEntry({ status: 429, provider: "openrouter", error_kind: "provider_error", error_message: "Provider returned error" })],
    });
    await renderApp(projectPath("logs?log=req_0001"));
    const outcome = section(await sheet(), "Outcome");
    expect(outcome).toHaveTextContent("Provider error");
    expect(await within(outcome).findByText("Message from OpenRouter")).toBeInTheDocument();
    expect(outcome).toHaveTextContent("Provider returned error");
  });

  it("leaves out a message that only restates the status", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()], entries: [logEntry({ status: 529, provider: "anthropic", error_kind: "overloaded", error_message: "Overloaded" })] });
    await renderApp(projectPath("logs?log=req_0001"));
    const outcome = section(await sheet(), "Outcome");
    expect(outcome).toHaveTextContent(/^Outcome\s*Status529(Overloaded)+$/);
  });
});
