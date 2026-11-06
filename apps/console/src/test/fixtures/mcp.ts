import type { McpConnectionView, McpServerProbeView, McpServerView, McpToolView } from "~/api/generated/types";
import { NOW, PROJECT, USER } from "../fixtures";

export const obj = (value: Record<string, unknown>) => value as Record<string, never>;

export const SERVERS = "orgs/:org/mcp-servers";
export const SERVER = `${SERVERS}/:server`;
export const SERVER_TOOLS = `${SERVER}/tools`;

export function mcpConnection(overrides: Partial<McpConnectionView> = {}): McpConnectionView {
  return {
    id: "conn_x",
    kind: "oauth",
    account: "@kaesoapp",
    last4: "a1b2",
    header_name: null,
    signed_out: false,
    updated_at: "2026-10-15T11:00:00Z",
    expires_at: "2026-10-15T13:00:00Z",
    last_used_at: null,
    ...overrides,
  };
}

export function mcpServer(overrides: Partial<McpServerView> = {}): McpServerView {
  return {
    id: "mcp_x",
    name: "X",
    handle: "x",
    url: "https://mcp.x.example/mcp",
    auth_method: "oauth",
    credential_mode: "shared",
    change_policy: "hold",
    connection: mcpConnection(),
    created_at: NOW,
    created_by: { kind: "user", id: USER },
    updated_at: NOW,
    last_checked_at: "2026-10-15T11:58:00Z",
    last_error: null,
    projects: [PROJECT],
    protocol_version: "2026-07-28",
    revision: 3,
    scopes: [],
    server_info: obj({ name: "x-mcp", version: "1.2.0" }),
    status: "ok",
    tools: { total: 3, changed: 1 },
    ...overrides,
  };
}

export function mcpTool(overrides: Partial<McpToolView> = {}): McpToolView {
  const name = overrides.name ?? "get_me";
  return {
    id: `tool_${name}`,
    name,
    exposed_name: `x__${name}`,
    title: null,
    description: `The ${name} tool.`,
    input_schema: obj({ type: "object" }),
    output_schema: null,
    annotations: obj({ readOnlyHint: true }),
    state: "active",
    hash: "a".repeat(64),
    approved_hash: "a".repeat(64),
    long_description: false,
    first_seen_at: NOW,
    changed_at: null,
    ...overrides,
  };
}

export const PROBED_TOOLS = [
  { name: "create_post", title: "Create a post", description: "Posts to X.", annotations: obj({ destructiveHint: true }) },
  { name: "get_me", title: null, description: "Reads your profile.", annotations: obj({ readOnlyHint: true }) },
  { name: "search", title: null, description: null, annotations: obj({ readOnlyHint: true }) },
];

export function probeView(overrides: Partial<McpServerProbeView> = {}): McpServerProbeView {
  return {
    url: "https://mcp.x.example/mcp",
    problem: null,
    http_status: null,
    protocol_version: "2026-07-28",
    server_info: obj({ name: "mock-x", version: "0.1.0" }),
    sign_in: { method: "none", issuer: null, client_metadata_documents: false, registration: false, scopes: [] },
    tools: PROBED_TOOLS,
    handle: "mock-x",
    ...overrides,
  };
}
