import { describe, expect, it } from "vitest";
import { mcpConnection, mcpServer, projectServer } from "~/test/fixtures/mcp";
import {
  connectionWord,
  fatalProblem,
  hint,
  hostOf,
  problemText,
  projectServerWord,
  serverInfoText,
  signInLabel,
  statusWord,
  toolStateWord,
} from "./mcp-servers";

describe("MCP server words", () => {
  it("names how Kaeso is connected to a server", () => {
    const words = [
      mcpServer(),
      mcpServer({ connection: mcpConnection({ account: null }) }),
      mcpServer({ auth_method: "header", connection: mcpConnection({ kind: "header", header_name: "Authorization", account: null }) }),
      mcpServer({ auth_method: "header", connection: mcpConnection({ kind: "header", header_name: null, account: null }) }),
      mcpServer({ auth_method: "none", connection: null }),
      mcpServer({ credential_mode: "per_user", connection: null }),
      mcpServer({ connection: null }),
      mcpServer({ connection: mcpConnection({ signed_out: true }) }),
      mcpServer({ status: "signed_out" }),
    ].map((s) => connectionWord(s).label);
    expect(words).toEqual([
      "Connected as @kaesoapp",
      "Connected",
      "Sends Authorization",
      "Sends ",
      "No sign-in",
      "Each person signs in",
      "Not signed in",
      "Signed out",
      "Signed out",
    ]);
  });

  it("names how a project's server is connected", () => {
    expect(
      [
        projectServer(),
        projectServer({ connected: false }),
        projectServer({ credential_mode: "per_user", connected: true }),
        projectServer({ auth_method: "none" }),
      ].map((s) => projectServerWord(s).label),
    ).toEqual(["Connected", "Signed out", "Each person signs in", "No sign-in"]);
  });

  it("names the last check, a tool's state and a sign-in method", () => {
    expect((["ok", "unreachable", "signed_out", "error", "unchecked"] as const).map((s) => statusWord(s).label)).toEqual([
      "Working",
      "Unreachable",
      "Signed out",
      "Answered with an error",
      "Not checked yet",
    ]);
    expect((["active", "changed", "removed"] as const).map((s) => toolStateWord(s).label)).toEqual(["Active", "Changed", "Removed"]);
    expect((["none", "header", "oauth"] as const).map(signInLabel)).toEqual(["No sign-in", "Secret header", "OAuth"]);
  });

  it("explains why an address couldn't be used", () => {
    const problems = (["refused", "unreachable", "timeout", "status", "not_mcp", "header_refused", "sign_in", null] as const).map((problem) =>
      problemText({ problem, http_status: problem === "status" ? 502 : null }),
    );
    expect(problems).toEqual([
      "This install doesn't allow this address.",
      "Nothing answered at this address; check it and try again.",
      "The server didn't answer in time; try again.",
      "The server answered with HTTP status 502.",
      "The server's answer isn't MCP; check the address.",
      "The server refused the header; check its name and value.",
      "The server asks for an OAuth sign-in Kaeso can't do; try a secret header.",
      null,
    ]);
    expect(problemText({ problem: "status", http_status: null })).toBe("The server answered with HTTP status 0.");
    expect(fatalProblem("not_mcp")).toBe(true);
    expect(fatalProblem("header_refused")).toBe(false);
    expect(fatalProblem(null)).toBe(false);
  });

  it("reads hosts, what a server says about itself and tool hints", () => {
    expect(hostOf("https://mcp.x.example/mcp")).toBe("mcp.x.example");
    expect(hostOf("not an address")).toBe("not an address");
    expect(serverInfoText({ title: "X", name: "x-mcp", version: "1.2.0" })).toBe("X 1.2.0");
    expect(serverInfoText({ name: "x-mcp" })).toBe("x-mcp");
    expect(serverInfoText({ version: "1" })).toBeNull();
    expect(serverInfoText(null)).toBeNull();
    expect(serverInfoText("x")).toBeNull();
    expect(hint({ readOnlyHint: true }, "readOnlyHint")).toBe(true);
    expect(hint({ readOnlyHint: "yes" }, "readOnlyHint")).toBe(false);
    expect(hint(null, "destructiveHint")).toBe(false);
  });
});
