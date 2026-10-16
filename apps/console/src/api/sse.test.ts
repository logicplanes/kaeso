import { describe, expect, it } from "vitest";
import { SseParser, readSse } from "./sse";

const all = (chunks: string[]) => {
  const p = new SseParser();
  return [...chunks.flatMap((c) => p.push(c)), ...p.end()];
};

describe("SseParser", () => {
  it("reads events with their type and data", () => {
    expect(all(['event: response.created\ndata: {"a":1}\n\nevent: response.completed\ndata: {"b":2}\n\n'])).toEqual([
      { event: "response.created", data: '{"a":1}' },
      { event: "response.completed", data: '{"b":2}' },
    ]);
  });

  it("puts an event back together wherever the chunks cut it", () => {
    const stream = 'event: response.output_text.delta\ndata: {"delta":"héllo"}\n\nevent: response.completed\ndata: {}\n\n';
    const whole = all([stream]);
    for (let i = 1; i < stream.length; i++) expect(all([stream.slice(0, i), stream.slice(i)])).toEqual(whole);
    expect(all([...stream])).toEqual(whole);
  });

  it("accepts CRLF and CR line breaks, also split between chunks", () => {
    expect(all(["event: a\r", "\ndata: 1\r\n\r", "\nevent: b\rdata: 2\r\r"])).toEqual([
      { event: "a", data: "1" },
      { event: "b", data: "2" },
    ]);
  });

  it("skips keepalive comments and counts them", () => {
    const p = new SseParser();
    const events = [...p.push(": keepalive\n\n:\n\ndata: x\n"), ...p.push(": keepalive\n\n")];
    expect(events).toEqual([{ event: "message", data: "x" }]);
    expect(p.comments).toBe(3);
  });

  it("joins data lines, strips one leading space and keeps the id", () => {
    expect(all(["id: 7\ndata:  two spaces\ndata:line\ndata\n\n"])).toEqual([{ event: "message", data: " two spaces\nline\n", id: "7" }]);
  });

  it("ignores blank events and unknown fields, and keeps a last event without its blank line", () => {
    expect(all(["\n\nretry: 10\nfoo: bar\n\nevent: error\ndata: {}"])).toEqual([{ event: "error", data: "{}" }]);
  });
});

describe("readSse", () => {
  it("decodes bytes split inside a character", async () => {
    const bytes = new TextEncoder().encode('data: {"delta":"日本"}\n\n');
    const body = new ReadableStream<Uint8Array>({
      start(c) {
        c.enqueue(bytes.slice(0, 13));
        c.enqueue(bytes.slice(13));
        c.close();
      },
    });
    const out = [];
    for await (const e of readSse(body)) out.push(e);
    expect(out).toEqual([{ event: "message", data: '{"delta":"日本"}' }]);
  });

  it("stops reading when the signal aborts, even when the stream refuses to cancel", async () => {
    const controller = new AbortController();
    const body = new ReadableStream<Uint8Array>({
      start(c) {
        c.enqueue(new TextEncoder().encode("data: first\n\n"));
      },
      cancel() {
        throw new Error("can't cancel");
      },
    });
    const out = [];
    for await (const e of readSse(body, controller.signal)) {
      out.push(e);
      controller.abort();
    }
    expect(out).toEqual([{ event: "message", data: "first" }]);
  });
});
