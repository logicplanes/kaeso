import { randomUUID } from "node:crypto";
import { createServer } from "node:http";

const PORT = Number(process.env.PORT ?? 8080);
const MODERN = "2026-07-28";
const LEGACY = ["2025-11-25", "2025-06-18", "2025-03-26"];
const SERVER_INFO = { name: "mock-x", version: "2.0.0" };

const ERAS = {
  "/mcp": { modern: true, legacy: true },
  "/modern/mcp": { modern: true, legacy: false },
  "/legacy/mcp": { modern: false, legacy: true },
};

const tools = [
  {
    name: "create_post",
    title: "Create post",
    description: "Posts a message to X as the signed-in account.",
    inputSchema: { type: "object", properties: { text: { type: "string" } }, required: ["text"] },
    annotations: { destructiveHint: true, openWorldHint: true },
  },
  {
    name: "search_posts",
    title: "Search posts",
    description: "Finds recent posts that match a query.",
    inputSchema: { type: "object", properties: { query: { type: "string" } }, required: ["query"] },
    outputSchema: { type: "object", properties: { posts: { type: "array" } } },
    annotations: { readOnlyHint: true },
  },
  {
    name: "get_me",
    description: "Shows the signed-in account.",
    inputSchema: { type: "object" },
    annotations: { readOnlyHint: true },
  },
];

const sessions = new Set();

function send(res, status, body, headers = {}) {
  res.writeHead(status, { "content-type": "application/json", "cache-control": "no-store", ...headers });
  res.end(JSON.stringify(body));
}

const error = (id, code, message, data) => ({ jsonrpc: "2.0", id, error: { code, message, ...(data ? { data } : {}) } });

function events(res, messages) {
  res.writeHead(200, { "content-type": "text/event-stream", "cache-control": "no-store", "x-accel-buffering": "no" });
  for (const m of messages) res.write(`event: message\ndata: ${JSON.stringify(m)}\n\n`);
  res.end();
}

async function read(req) {
  const chunks = [];
  for await (const chunk of req) chunks.push(chunk);
  return Buffer.concat(chunks).toString("utf8");
}

function call(id, params, modern) {
  const tool = tools.find((t) => t.name === params.name);
  if (!tool) return error(id, -32602, `Unknown tool: ${params.name}`);
  const text = params.name === "create_post" ? `Posted: ${params.arguments?.text ?? ""}` : `${params.name} ran.`;
  return { jsonrpc: "2.0", id, result: { content: [{ type: "text", text }], isError: false, ...(modern ? { resultType: "complete" } : {}) } };
}

async function handle(req, res) {
  const url = new URL(req.url ?? "/", "http://mcp");
  if (url.pathname === "/healthz") return send(res, 200, { ok: true });
  if (url.pathname === "/admin/tools" && req.method === "POST") {
    const change = JSON.parse((await read(req)) || "{}");
    const tool = tools.find((t) => t.name === change.name);
    if (tool && typeof change.description === "string") tool.description = change.description;
    return send(res, tool ? 200 : 404, tool ?? {});
  }
  const era = ERAS[url.pathname];
  if (!era) return send(res, 404, { error: "not found" });
  if (req.method !== "POST") {
    res.writeHead(405, { allow: "POST" });
    return res.end();
  }
  let message;
  try {
    message = JSON.parse(await read(req));
  } catch {
    return send(res, 400, error(null, -32700, "Parse error"));
  }
  if (message.id === undefined) {
    res.writeHead(202);
    return res.end();
  }
  const { id, method } = message;
  const params = message.params ?? {};
  const declared = params._meta?.["io.modelcontextprotocol/protocolVersion"];
  if (method === "initialize") {
    if (!era.legacy) return send(res, 400, error(id, -32022, "Unsupported protocol version", { supported: [MODERN], requested: params.protocolVersion }));
    const version = LEGACY.includes(params.protocolVersion) ? params.protocolVersion : LEGACY[0];
    const session = randomUUID();
    sessions.add(session);
    const result = { protocolVersion: version, capabilities: { tools: { listChanged: false } }, serverInfo: SERVER_INFO, instructions: "Posts to X." };
    return send(res, 200, { jsonrpc: "2.0", id, result }, { "mcp-session-id": session });
  }
  const modern = declared !== undefined;
  if (modern) {
    if (!era.modern) return send(res, 400, error(null, -32000, "Bad Request: No valid session ID provided"));
    if (declared !== MODERN) return send(res, 400, error(id, -32022, "Unsupported protocol version", { supported: [MODERN], requested: declared }));
    if (req.headers["mcp-protocol-version"] !== MODERN || req.headers["mcp-method"] !== method) {
      return send(res, 400, error(id, -32020, "Header mismatch"));
    }
  } else if (method !== "server/discover" || !era.modern) {
    const session = req.headers["mcp-session-id"];
    if (!session) return send(res, 400, error(null, -32000, "Bad Request: No valid session ID provided"));
    if (!sessions.has(session)) return send(res, 404, error(id, -32001, "Session not found"));
  }
  if (method === "server/discover") {
    const supportedVersions = era.legacy ? [MODERN, ...LEGACY] : [MODERN];
    const result = {
      resultType: "complete",
      supportedVersions,
      capabilities: { tools: {} },
      _meta: { "io.modelcontextprotocol/serverInfo": SERVER_INFO },
      instructions: "Posts to X.",
      ttlMs: 3_600_000,
      cacheScope: "public",
    };
    return send(res, 200, { jsonrpc: "2.0", id, result });
  }
  if (method === "tools/list") {
    const result = { tools, ...(modern ? { resultType: "complete", ttlMs: 300_000, cacheScope: "private" } : {}) };
    return send(res, 200, { jsonrpc: "2.0", id, result });
  }
  if (method === "tools/call") {
    const progress = { jsonrpc: "2.0", method: "notifications/progress", params: { progressToken: params._meta?.progressToken ?? "p", progress: 1, total: 2 } };
    return events(res, [progress, call(id, params, modern)]);
  }
  if (method === "ping") return send(res, 200, { jsonrpc: "2.0", id, result: {} });
  return send(res, 404, error(id, -32601, "Method not found"));
}

createServer((req, res) => {
  handle(req, res).catch((e) => {
    console.error(e);
    if (!res.headersSent) send(res, 500, error(null, -32603, "The mock MCP server failed."));
    else res.end();
  });
}).listen(PORT, () => console.log(`mock MCP server on :${PORT}`));
