// The model API's JSON endpoints, by the endpoint the log names. Uploads (transcriptions, image
// edits and variations, videos, System One files), ElevenLabs calls, whose path names a voice
// the entry leaves out, and the `…/completed` entries, which no app sent, can't be rebuilt.
const JSON_PATHS: Readonly<Record<string, string>> = {
  "chat/completions": "/v1/chat/completions",
  responses: "/v1/responses",
  "responses/input_tokens": "/v1/responses/input_tokens",
  messages: "/v1/messages",
  "messages/count_tokens": "/v1/messages/count_tokens",
  embeddings: "/v1/embeddings",
  "images/generations": "/v1/images/generations",
  "audio/speech": "/v1/audio/speech",
  rerank: "/v2/rerank",
};

const GEMINI_METHODS = new Set(["generateContent", "streamGenerateContent", "countTokens", "embedContent", "batchEmbedContents"]);

const isObj = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);

// Media the log left out is a placeholder, not the bytes the app sent.
function holdsLeftOutMedia(v: unknown): boolean {
  if (Array.isArray(v)) return v.some(holdsLeftOutMedia);
  if (!isObj(v)) return false;
  return "kaeso_media" in v || Object.values(v).some(holdsLeftOutMedia);
}

/** What of a log entry the command is built from. */
export interface Logged {
  endpoint: string;
  ingress_dialect: string;
  requested_model: string;
  content?: { request?: unknown } | null | undefined;
}

function pathOf(e: Logged): string | null {
  if (e.endpoint === "decisions") return e.ingress_dialect === "gemini_decisions" ? "/v1beta/decisions" : "/v1/decisions";
  const method = e.endpoint.startsWith("models:") ? e.endpoint.slice("models:".length) : null;
  if (method !== null) {
    if (!GEMINI_METHODS.has(method) || !e.requested_model) return null;
    return `/v1beta/models/${encodeURIComponent(e.requested_model)}:${method}${method === "streamGenerateContent" ? "?alt=sse" : ""}`;
  }
  return JSON_PATHS[e.endpoint] ?? null;
}

const quoted = (text: string) => `'${text.replaceAll("'", `'\\''`)}'`;

/** A `curl` command that sends a logged request again, with the key read from `KAESO_API_KEY`,
 * or `null` when the entry doesn't hold all of the request. */
export function curlCommand(e: Logged, origin: string): string | null {
  const body = e.content?.request;
  const path = pathOf(e);
  if (!path || !isObj(body) || holdsLeftOutMedia(body)) return null;
  const streams = body.stream === true || path.includes("?alt=sse");
  return [
    `curl ${streams ? "-N " : ""}${quoted(`${origin}${path}`)}`,
    `  -H "Authorization: Bearer $KAESO_API_KEY"`,
    `  -H 'Content-Type: application/json'`,
    `  -d ${quoted(JSON.stringify(body))}`,
  ].join(" \\\n");
}
