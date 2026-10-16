export interface SseEvent {
  event: string;
  data: string;
  id?: string;
}

export class SseParser {
  private buffer = "";
  private pendingCr = false;
  private event = "";
  private data: string[] = [];
  private id: string | undefined;
  comments = 0;

  push(chunk: string): SseEvent[] {
    let text = chunk;
    if (this.pendingCr && text.startsWith("\n")) text = text.slice(1);
    this.pendingCr = false;
    this.buffer += text;
    const out: SseEvent[] = [];
    let start = 0;
    for (let i = 0; i < this.buffer.length; i++) {
      const c = this.buffer[i];
      if (c !== "\n" && c !== "\r") continue;
      this.line(this.buffer.slice(start, i), out);
      if (c === "\r") {
        if (i + 1 === this.buffer.length) this.pendingCr = true;
        else if (this.buffer[i + 1] === "\n") i++;
      }
      start = i + 1;
    }
    this.buffer = this.buffer.slice(start);
    return out;
  }

  end(): SseEvent[] {
    const out: SseEvent[] = [];
    if (this.buffer) this.line(this.buffer, out);
    this.buffer = "";
    this.line("", out);
    return out;
  }

  private line(line: string, out: SseEvent[]): void {
    if (line === "") {
      if (this.data.length) out.push({ event: this.event || "message", data: this.data.join("\n"), ...(this.id !== undefined ? { id: this.id } : {}) });
      this.event = "";
      this.data = [];
      return;
    }
    if (line.startsWith(":")) {
      this.comments++;
      return;
    }
    const colon = line.indexOf(":");
    const field = colon === -1 ? line : line.slice(0, colon);
    let value = colon === -1 ? "" : line.slice(colon + 1);
    if (value.startsWith(" ")) value = value.slice(1);
    if (field === "event") this.event = value;
    else if (field === "data") this.data.push(value);
    else if (field === "id" && !value.includes("\0")) this.id = value;
  }
}

export async function* readSse(body: ReadableStream<Uint8Array>, signal?: AbortSignal): AsyncGenerator<SseEvent> {
  const reader = body.getReader();
  const decoder = new TextDecoder();
  const parser = new SseParser();
  const cancel = () => void reader.cancel().catch(() => undefined);
  signal?.addEventListener("abort", cancel, { once: true });
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      yield* parser.push(decoder.decode(value, { stream: true }));
    }
    yield* parser.push(decoder.decode());
    yield* parser.end();
  } finally {
    signal?.removeEventListener("abort", cancel);
    reader.releaseLock();
  }
}
