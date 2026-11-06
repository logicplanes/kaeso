import type { McpProbeProblem, McpServerProbeView, McpServerStatus, McpServerView, McpToolState, ProjectMcpServerView } from "~/api/generated/types";
import { t } from "~/i18n";

type Tone = "success" | "warning" | "danger" | "info" | "neutral";
export type Word = { tone: Tone; label: string };

export const HANDLE = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
export const MAX_HANDLE = 24;

// Clients take only `[a-zA-Z0-9_-]`, and Kaeso writes a handle's hyphens as underscores in tool names.
export const toolPrefix = (handle: string): string => `${(handle || "x").replace(/-/g, "_")}__`;

export const portalUrl = (project: string): string => `${window.location.origin}/mcp/${project}`;

export function hostOf(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}

export function connectionWord(s: McpServerView): Word {
  if (s.auth_method === "none") return { tone: "neutral", label: t("orgHome.mcp.connection.none", "No sign-in") };
  if (s.credential_mode === "per_user") return { tone: "neutral", label: t("orgHome.mcp.connection.perUser", "Each person signs in") };
  const c = s.connection;
  if (!c) return { tone: "warning", label: t("orgHome.mcp.connection.missing", "Not signed in") };
  if (c.signed_out || s.status === "signed_out") return { tone: "warning", label: t("orgHome.mcp.connection.signedOut", "Signed out") };
  if (c.kind === "header") return { tone: "success", label: t("orgHome.mcp.connection.header", "Sends {header}", { header: c.header_name ?? "" }) };
  return c.account
    ? { tone: "success", label: t("orgHome.mcp.connection.account", "Connected as {account}", { account: c.account }) }
    : { tone: "success", label: t("orgHome.mcp.connection.connected", "Connected") };
}

export function projectServerWord(s: ProjectMcpServerView): Word {
  if (s.auth_method === "none") return { tone: "neutral", label: t("orgHome.mcp.connection.none", "No sign-in") };
  if (s.credential_mode === "per_user") return { tone: "neutral", label: t("orgHome.mcp.connection.perUser", "Each person signs in") };
  return s.connected
    ? { tone: "success", label: t("orgHome.mcp.connection.connected", "Connected") }
    : { tone: "warning", label: t("orgHome.mcp.connection.signedOut", "Signed out") };
}

export type Hint = "read_only" | "destructive" | "open_world";

const HINT_KEYS: Record<Hint, "readOnlyHint" | "destructiveHint" | "openWorldHint"> = {
  read_only: "readOnlyHint",
  destructive: "destructiveHint",
  open_world: "openWorldHint",
};

export const HINTS: Hint[] = ["read_only", "destructive", "open_world"];

export const hintsOf = (annotations: unknown): Hint[] => HINTS.filter((h) => hint(annotations, HINT_KEYS[h]));

export function hintLabel(h: Hint): string {
  if (h === "read_only") return t("project.tools.hint.readOnly", "Read-only");
  if (h === "destructive") return t("project.tools.hint.destructive", "Destructive");
  return t("project.tools.hint.openWorld", "Open world");
}

export function statusWord(status: McpServerStatus): Word {
  switch (status) {
    case "ok":
      return { tone: "success", label: t("orgHome.mcp.status.ok", "Working") };
    case "unreachable":
      return { tone: "danger", label: t("orgHome.mcp.status.unreachable", "Unreachable") };
    case "signed_out":
      return { tone: "warning", label: t("orgHome.mcp.status.signedOut", "Signed out") };
    case "error":
      return { tone: "danger", label: t("orgHome.mcp.status.error", "Answered with an error") };
    default:
      return { tone: "neutral", label: t("orgHome.mcp.status.unchecked", "Not checked yet") };
  }
}

export function toolStateWord(state: McpToolState): Word {
  switch (state) {
    case "changed":
      return { tone: "warning", label: t("orgHome.mcp.tool.changed", "Changed") };
    case "removed":
      return { tone: "neutral", label: t("orgHome.mcp.tool.removed", "Removed") };
    default:
      return { tone: "success", label: t("orgHome.mcp.tool.active", "Active") };
  }
}

export function signInLabel(method: "none" | "header" | "oauth"): string {
  switch (method) {
    case "header":
      return t("orgHome.mcp.signIn.header", "Secret header");
    case "oauth":
      return t("orgHome.mcp.signIn.oauth", "OAuth");
    default:
      return t("orgHome.mcp.signIn.none", "No sign-in");
  }
}

export function problemText(probe: Pick<McpServerProbeView, "problem" | "http_status">): string | null {
  const problem: McpProbeProblem | null | undefined = probe.problem;
  switch (problem) {
    case "refused":
      return t("orgHome.mcp.problem.refused", "This install doesn't allow this address.");
    case "unreachable":
      return t("orgHome.mcp.problem.unreachable", "Nothing answered at this address; check it and try again.");
    case "timeout":
      return t("orgHome.mcp.problem.timeout", "The server didn't answer in time; try again.");
    case "status":
      return t("orgHome.mcp.problem.status", "The server answered with HTTP status {status}.", { status: probe.http_status ?? 0 });
    case "not_mcp":
      return t("orgHome.mcp.problem.notMcp", "The server's answer isn't MCP; check the address.");
    case "header_refused":
      return t("orgHome.mcp.problem.headerRefused", "The server refused the header; check its name and value.");
    case "sign_in":
      return t("orgHome.mcp.problem.signIn", "The server asks for an OAuth sign-in Kaeso can't do; try a secret header.");
    default:
      return null;
  }
}

export const fatalProblem = (problem: McpProbeProblem | null | undefined): boolean =>
  problem === "refused" || problem === "unreachable" || problem === "timeout" || problem === "status" || problem === "not_mcp";

export function serverInfoText(info: unknown): string | null {
  if (!info || typeof info !== "object") return null;
  const o = info as Record<string, unknown>;
  const name = typeof o.title === "string" ? o.title : typeof o.name === "string" ? o.name : null;
  if (!name) return null;
  return typeof o.version === "string" ? t("orgHome.mcp.serverVersion", "{name} {version}", { name, version: o.version }) : name;
}

export function hint(annotations: unknown, key: "readOnlyHint" | "destructiveHint" | "openWorldHint"): boolean {
  return !!annotations && typeof annotations === "object" && (annotations as Record<string, unknown>)[key] === true;
}
