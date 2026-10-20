import { afterAll, describe, expect, it, vi, type Mock } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";

let fetchMock: Mock = vi.fn();
vi.stubGlobal("fetch", (...args: unknown[]) => fetchMock(...args) as unknown);
const { apply, load } = await import("~/i18n");
const { useThreads } = await import("./use-threads");

const event = (type: string, data: Record<string, unknown>) => `event: ${type}\ndata: ${JSON.stringify({ type, ...data })}\n\n`;

function streamingFetch() {
  const encoder = new TextEncoder();
  let push: (text: string) => void = () => {};
  let close: () => void = () => {};
  let fail: (e: Error) => void = () => {};
  let cancelled = false;
  const fetch = vi.fn(async (input: Request) => {
    const body = new ReadableStream<Uint8Array>({
      start(c) {
        push = (t) => c.enqueue(encoder.encode(t));
        close = () => c.close();
        fail = (e) => c.error(e);
        input.signal.addEventListener("abort", () => c.close());
      },
      cancel() {
        cancelled = true;
      },
    });
    return new Response(body, { status: 200, headers: { "content-type": "text/event-stream" } });
  });
  return { fetch, push: (t: string) => push(t), close: () => close(), fail: (e: Error) => fail(e), cancelled: () => cancelled };
}

const request = { model: "gpt-5.4-mini", input: [{ role: "user", content: "Hi" }], stream: true as const };

afterAll(() => vi.unstubAllGlobals());

describe("useThreads", () => {
  it("streams a reply into its column and ends it with the usage", async () => {
    const s = streamingFetch();
    fetchMock = s.fetch;
    const { result } = renderHook(() => useThreads("org_1", "proj_1", 1));
    act(() => result.current.send("Hi", [{ column: 0, request }]));
    await waitFor(() => expect(s.fetch).toHaveBeenCalled());
    const sent = s.fetch.mock.calls[0]?.[0] as Request;
    expect(sent.url).toMatch(/\/platform\/v1\/orgs\/org_1\/projects\/proj_1\/playground\/responses$/);
    expect(await sent.clone().json()).toEqual(request);
    expect(result.current.busy).toBe(true);
    act(() => s.push(": keepalive\n\n" + event("response.output_text.delta", { item_id: "m", delta: "Hel" })));
    act(() => s.push(event("response.output_text.delta", { item_id: "m", delta: "lo." })));
    await waitFor(() => expect(result.current.threads[0]?.turns[0]?.response.text).toBe("Hello."));
    act(() => s.push(event("response.completed", { response: { usage: { input_tokens: 3, output_tokens: 2 } } })));
    await waitFor(() => expect(result.current.threads[0]?.turns[0]?.response.status).toBe("completed"));
    expect(result.current.threads[0]?.turns[0]?.response.usage).toMatchObject({ input_tokens: 3, output_tokens: 2 });
    expect(result.current.busy).toBe(false);
  });

  it("reads a finished reply's stream to its end instead of hanging up on the gateway", async () => {
    const s = streamingFetch();
    fetchMock = s.fetch;
    const { result } = renderHook(() => useThreads("org_1", "proj_1", 2));
    act(() => result.current.send("Hi", [{ column: 0, request }]));
    await waitFor(() => expect(s.fetch).toHaveBeenCalled());
    const sent = s.fetch.mock.calls[0]?.[0] as Request;
    act(() => s.push(event("response.output_text.delta", { item_id: "m", delta: "Done." }) + event("response.completed", { response: {} })));
    await waitFor(() => expect(result.current.threads[0]?.turns[0]?.response.status).toBe("completed"));
    expect(result.current.busy).toBe(false);
    act(() => result.current.stop());
    act(() => s.push(": keepalive\n\n" + event("response.output_text.delta", { item_id: "m", delta: " Late." })));
    act(() => s.close());
    await act(async () => {});
    expect(s.cancelled()).toBe(false);
    expect(sent.signal.aborted).toBe(false);
    expect(result.current.threads[0]?.turns[0]?.response).toMatchObject({ status: "completed", text: "Done.", error: null });
  });

  it("stops every stream and keeps what arrived, without an error", async () => {
    const s = streamingFetch();
    fetchMock = s.fetch;
    const { result } = renderHook(() => useThreads("org_1", "proj_1", 1));
    act(() => result.current.send("Hi", [{ column: 0, request }]));
    await waitFor(() => expect(s.fetch).toHaveBeenCalled());
    act(() => s.push(event("response.output_text.delta", { item_id: "m", delta: "Partial" })));
    await waitFor(() => expect(result.current.threads[0]?.turns[0]?.response.text).toBe("Partial"));
    act(() => result.current.stop());
    await waitFor(() => expect(result.current.threads[0]?.turns[0]?.response.status).toBe("stopped"));
    expect(result.current.threads[0]?.turns[0]?.response.text).toBe("Partial");
    expect(result.current.threads[0]?.turns[0]?.response.error).toBeNull();
  });

  it("keeps a refusal's message on the column it belongs to", async () => {
    fetchMock = vi.fn(
      async () =>
        new Response(JSON.stringify({ error: { type: "rate_limited", message: "Budget used up: the project's $5.00 per month." } }), {
          status: 429,
          headers: { "content-type": "application/json" },
        }),
    );
    const { result } = renderHook(() => useThreads("org_1", "proj_1", 2));
    act(() => result.current.send("Hi", [{ column: 1, request }]));
    await waitFor(() => expect(result.current.threads[1]?.turns[0]?.response.status).toBe("failed"));
    expect(result.current.threads[0]?.turns).toEqual([]);
    expect(result.current.threads[1]?.turns[0]?.response).toMatchObject({ error: "Budget used up: the project's $5.00 per month.", requestId: null });
  });

  it("skips events it can't read and fails a stream that ends too early", async () => {
    const s = streamingFetch();
    fetchMock = s.fetch;
    const { result } = renderHook(() => useThreads("org_1", "proj_1", 1));
    act(() => result.current.send("Hi", [{ column: 0, request }]));
    await waitFor(() => expect(s.fetch).toHaveBeenCalled());
    act(() =>
      s.push(
        "event: response.output_text.delta\ndata: {broken\n\n" +
          "event: x\ndata: null\n\n" +
          event("response.output_text.delta", { item_id: "m", delta: "Half" }),
      ),
    );
    await waitFor(() => expect(result.current.threads[0]?.turns[0]?.response.text).toBe("Half"));
    act(() => s.close());
    await waitFor(() => expect(result.current.threads[0]?.turns[0]?.response.status).toBe("failed"));
    expect(result.current.threads[0]?.turns[0]?.response.error).toBe("The stream ended before the model finished. Try again.");
  });

  it("names the request id of a gateway failure and says when the connection drops", async () => {
    fetchMock = vi.fn(
      async () =>
        new Response(JSON.stringify({ error: { type: "internal", message: "The gateway failed." } }), {
          status: 502,
          headers: { "content-type": "application/json", "kaeso-request-id": "req_abc" },
        }),
    );
    const { result } = renderHook(() => useThreads("org_1", "proj_1", 1));
    act(() => result.current.send("Hi", [{ column: 0, request }]));
    await waitFor(() => expect(result.current.threads[0]?.turns[0]?.response.status).toBe("failed"));
    expect(result.current.threads[0]?.turns[0]?.response).toMatchObject({ error: "The gateway failed.", requestId: expect.stringMatching(/\S+/) });

    fetchMock = vi.fn(async () => {
      throw new TypeError("offline");
    });
    act(() => result.current.send("Again", [{ column: 0, request }]));
    await waitFor(() => expect(result.current.threads[0]?.turns.at(-1)?.response.status).toBe("failed"));
    expect(result.current.threads[0]?.turns.at(-1)?.response).toMatchObject({ error: "Can't reach the gateway. Try again.", requestId: null });
  });

  it("translates the model's error into the person's language", async () => {
    apply(await load("de"));
    fetchMock = vi.fn(
      async () =>
        new Response(event("response.failed", { response: { error: { message: "Something went wrong; try again." } } }), {
          status: 200,
          headers: { "content-type": "text/event-stream" },
        }),
    );
    const { result } = renderHook(() => useThreads("org_1", "proj_1", 1));
    act(() => result.current.send("Hi", [{ column: 0, request }]));
    await waitFor(() => expect(result.current.threads[0]?.turns[0]?.response.error).toBe("Etwas ist schiefgelaufen; bitte erneut versuchen."));
  });

  it("retries the last turn, grows and shrinks columns, removes and clears them", async () => {
    let calls = 0;
    fetchMock = vi.fn(async () => {
      calls++;
      return new Response(
        calls === 1
          ? event("response.failed", { response: { error: { message: "Nope." } } })
          : event("response.output_text.delta", { item_id: "m", delta: "Yes." }) + event("response.completed", { response: {} }),
        { status: 200, headers: { "content-type": "text/event-stream" } },
      );
    });
    const { result, rerender } = renderHook(({ columns }) => useThreads("org_1", "proj_1", columns), { initialProps: { columns: 1 } });
    act(() => result.current.retry(0, request));
    act(() => result.current.send("Hi", [{ column: 0, request }]));
    await waitFor(() => expect(result.current.threads[0]?.turns[0]?.response.status).toBe("failed"));
    act(() => result.current.retry(0, request));
    await waitFor(() => expect(result.current.threads[0]?.turns[0]?.response.text).toBe("Yes."));
    expect(result.current.threads[0]?.turns).toHaveLength(1);
    expect(fetchMock).toHaveBeenCalledTimes(2);

    const firstKey = result.current.threads[0]?.key;
    act(() => {
      result.current.add();
      rerender({ columns: 2 });
    });
    expect(result.current.threads).toHaveLength(2);
    expect(result.current.threads[0]?.key).toBe(firstKey);
    rerender({ columns: 3 });
    expect(result.current.threads).toHaveLength(3);
    rerender({ columns: 2 });
    expect(result.current.threads).toHaveLength(2);
    const second = result.current.threads[1]?.key;
    act(() => {
      result.current.remove(0);
      rerender({ columns: 1 });
    });
    expect(result.current.threads.map((t) => t.key)).toEqual([second]);
    act(() => result.current.clear());
    expect(result.current.threads.every((t) => t.turns.length === 0)).toBe(true);
  });

  it("stops a column's stream when the column goes, and every stream on unmount", async () => {
    const s = streamingFetch();
    fetchMock = s.fetch;
    const { result, unmount } = renderHook(() => useThreads("org_1", "proj_1", 2));
    act(() => result.current.send("Hi", [{ column: 0, request }]));
    await waitFor(() => expect(s.fetch).toHaveBeenCalled());
    const sent = s.fetch.mock.calls[0]?.[0] as Request;
    act(() => result.current.remove(0));
    await waitFor(() => expect(sent.signal.aborted).toBe(true));

    const t = streamingFetch();
    fetchMock = t.fetch;
    act(() => result.current.send("Again", [{ column: 0, request }]));
    await waitFor(() => expect(t.fetch).toHaveBeenCalled());
    const again = t.fetch.mock.calls[0]?.[0] as Request;
    unmount();
    expect(again.signal.aborted).toBe(true);
  });

  it("stops a reply the gateway hasn't answered yet, without an error", async () => {
    fetchMock = vi.fn(
      (input: Request) =>
        new Promise<Response>((_, reject) => input.signal.addEventListener("abort", () => reject(new DOMException("The request was aborted.", "AbortError")))),
    );
    const { result } = renderHook(() => useThreads("org_1", "proj_1", 1));
    act(() => result.current.send("Hi", [{ column: 0, request }]));
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    act(() => result.current.stop());
    await waitFor(() => expect(result.current.threads[0]?.turns[0]?.response.status).toBe("stopped"));
    expect(result.current.threads[0]?.turns[0]?.response.error).toBeNull();
  });

  it("says the connection closed when the stream breaks off mid-reply", async () => {
    const s = streamingFetch();
    fetchMock = s.fetch;
    const { result } = renderHook(() => useThreads("org_1", "proj_1", 1));
    act(() => result.current.send("Hi", [{ column: 0, request }]));
    await waitFor(() => expect(s.fetch).toHaveBeenCalled());
    act(() => s.push(event("response.output_text.delta", { item_id: "m", delta: "Half" })));
    await waitFor(() => expect(result.current.threads[0]?.turns[0]?.response.text).toBe("Half"));
    act(() => s.fail(new TypeError("network error")));
    await waitFor(() => expect(result.current.threads[0]?.turns[0]?.response.status).toBe("failed"));
    expect(result.current.threads[0]?.turns[0]?.response).toMatchObject({ text: "Half", error: "The connection closed before the reply finished. Try again." });
  });

  it("retries one column and leaves the others as they are", async () => {
    const calls: Record<string, number> = {};
    fetchMock = vi.fn(async (input: Request) => {
      const { model } = (await input.clone().json()) as { model: string };
      calls[model] = (calls[model] ?? 0) + 1;
      const body =
        model === "a" && calls[model] === 1
          ? event("response.failed", { response: { error: { message: "Nope." } } })
          : event("response.output_text.delta", { item_id: "m", delta: `From ${model}.` }) + event("response.completed", { response: {} });
      return new Response(body, { status: 200, headers: { "content-type": "text/event-stream" } });
    });
    const { result } = renderHook(() => useThreads("org_1", "proj_1", 2));
    act(() =>
      result.current.send("Hi", [
        { column: 0, request: { ...request, model: "a" } },
        { column: 1, request: { ...request, model: "b" } },
      ]),
    );
    await waitFor(() => expect(result.current.threads[0]?.turns[0]?.response.status).toBe("failed"));
    await waitFor(() => expect(result.current.threads[1]?.turns[0]?.response.text).toBe("From b."));
    const other = result.current.threads[1];
    act(() => result.current.retry(0, { ...request, model: "a" }));
    await waitFor(() => expect(result.current.threads[0]?.turns[0]?.response.text).toBe("From a."));
    expect(result.current.threads[1]).toBe(other);
  });

  it("drops a repaint still waiting for its frame when it unmounts", async () => {
    const frames = vi.spyOn(globalThis, "requestAnimationFrame").mockImplementation(() => 42);
    const cancel = vi.spyOn(globalThis, "cancelAnimationFrame").mockImplementation(() => undefined);
    const s = streamingFetch();
    fetchMock = s.fetch;
    const { result, unmount } = renderHook(() => useThreads("org_1", "proj_1", 1));
    act(() => result.current.send("Hi", [{ column: 0, request }]));
    await waitFor(() => expect(s.fetch).toHaveBeenCalled());
    act(() => s.push(event("response.output_text.delta", { item_id: "m", delta: "Hel" })));
    await waitFor(() => expect(frames).toHaveBeenCalled());
    unmount();
    expect(cancel).toHaveBeenCalledWith(42);
    frames.mockRestore();
    cancel.mockRestore();
  });
});
