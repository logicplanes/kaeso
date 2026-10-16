import { describe, expect, it } from "vitest";
import { apiError, json, on, requests } from "~/test/network";
import { eventStream, sse } from "~/test/fixtures/project-logs";
import { streamPlayground, type PlaygroundRequest } from "./playground";

const body: PlaygroundRequest = { model: "gpt-5.4-mini", input: [{ role: "user", content: "Hi" }], temperature: 0.2, stream: true };
const PATH = "orgs/:org/projects/:project/playground/responses";

async function collect(signal = new AbortController().signal) {
  const out: { event: string; data: string }[] = [];
  for await (const e of streamPlayground("org_1", "proj_1", body, signal)) out.push({ event: e.event, data: e.data });
  return out;
}

describe("streamPlayground", () => {
  it("posts the request for a stream and yields its events in order", async () => {
    on("POST", PATH, eventStream(sse([{ type: "response.created" }, { type: "response.output_text.delta", delta: "Hi" }])));
    const events = await collect();
    expect(events.map((e) => e.event)).toEqual(["response.created", "response.output_text.delta"]);
    expect(JSON.parse(events[1]?.data ?? "{}")).toMatchObject({ delta: "Hi" });
    const [sent] = requests("POST", PATH);
    expect(sent?.params).toEqual({ org: "org_1", project: "proj_1" });
    expect(sent?.body).toEqual(body);
    expect(sent?.headers.get("accept")).toBe("text/event-stream");
  });

  it("refuses an answer that is not a stream, with the request id", async () => {
    on("POST", PATH, json({ id: "resp_1" }, { headers: { "kaeso-request-id": "req_42" } }));
    await expect(collect()).rejects.toMatchObject({
      type: "internal",
      status: 200,
      message: "The gateway sent an answer the playground can't read. Try again.",
      requestId: "req_42",
    });
  });

  it("passes on the gateway's refusal", async () => {
    on("POST", PATH, apiError(429, "rate_limited", "Budget used up: the project's $5.00 per month."));
    await expect(collect()).rejects.toMatchObject({ type: "rate_limited", status: 429, message: "Budget used up: the project's $5.00 per month." });
  });

  it("refuses an answer without a content type and lets its body go", async () => {
    let cancelled = false;
    on(
      "POST",
      PATH,
      () =>
        new Response(
          new ReadableStream({
            cancel() {
              cancelled = true;
              throw new Error("can't cancel");
            },
          }),
          { status: 200 },
        ),
    );
    await expect(collect()).rejects.toMatchObject({ message: "The gateway sent an answer the playground can't read. Try again." });
    expect(cancelled).toBe(true);
  });
});
