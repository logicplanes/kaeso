import type { App, McpUiHostCapabilities, McpUiHostContext } from "@modelcontextprotocol/ext-apps";
import type { CallToolResult } from "@modelcontextprotocol/sdk/types.js";
import { askOf, outcomeOf, safeUrl, TOOL, type Ask, type Failure, type Stats } from "./data";

export interface State {
  connected: boolean;
  context: McpUiHostContext;
  capabilities: McpUiHostCapabilities;
  args: Record<string, unknown>;
  ask: Ask;
  stats: Stats | null;
  failure: Failure | null;
  loading: boolean;
}

export type Host = Pick<
  App,
  | "callServerTool"
  | "openLink"
  | "requestDisplayMode"
  | "updateModelContext"
  | "getHostContext"
  | "getHostCapabilities"
  | "ontoolinput"
  | "ontoolresult"
  | "ontoolcancelled"
  | "onhostcontextchanged"
  | "onteardown"
>;

export interface Store {
  get: () => State;
  subscribe: (listener: () => void) => () => void;
  connected: () => void;
  failed: (error: unknown) => void;
  change: (next: Ask) => Promise<void>;
  open: (url: string) => Promise<void>;
  expand: () => Promise<void>;
}

const INITIAL: State = { connected: false, context: {}, capabilities: {}, args: {}, ask: {}, stats: null, failure: null, loading: true };

// Handlers go on before connect(): the host sends the tool's input and result right after the
// handshake, and a notification without a handler is lost (SEP-1865, "Lifecycle").
export function createStore(host: Host): Store {
  let state = INITIAL;
  let asked = 0;
  const listeners = new Set<() => void>();
  const set = (patch: Partial<State>) => {
    state = { ...state, ...patch };
    for (const l of listeners) l();
  };
  const show = (result: CallToolResult) => {
    const outcome = outcomeOf(result);
    if ("stats" in outcome)
      set({
        stats: outcome.stats,
        failure: null,
        loading: false,
        ask: { metric: outcome.stats.metric, range: outcome.stats.range, group_by: outcome.stats.group_by },
      });
    else set({ failure: outcome.failure, loading: false });
  };

  host.ontoolinput = ({ arguments: args }) => set({ args: args ?? {}, ask: askOf(args) });
  host.ontoolresult = (result) => {
    if (asked === 0) show(result);
  };
  host.ontoolcancelled = () => set({ failure: { kind: "cancelled" }, loading: false });
  host.onhostcontextchanged = (patch) => set({ context: { ...state.context, ...patch } });
  host.onteardown = () => ({});

  return {
    get: () => state,
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    connected: () => set({ connected: true, context: host.getHostContext() ?? {}, capabilities: host.getHostCapabilities() ?? {} }),
    failed: (error) => set({ failure: { kind: "error", message: error instanceof Error ? error.message : String(error) }, loading: false }),
    change: async (next) => {
      const ask = { ...state.ask, ...next };
      const call = ++asked;
      set({ ask, loading: true });
      const args = { ...state.args, ...ask };
      try {
        const result = await host.callServerTool({ name: TOOL, arguments: args });
        if (call !== asked) return;
        show(result);
        set({ args });
        const summary = result.content?.find((b) => b.type === "text");
        if (!result.isError && summary) void host.updateModelContext({ content: [summary] }).catch(() => undefined);
      } catch (error) {
        if (call === asked) set({ failure: { kind: "error", message: error instanceof Error ? error.message : String(error) }, loading: false });
      }
    },
    open: async (url) => {
      const safe = safeUrl(url);
      if (safe) await host.openLink({ url: safe });
    },
    expand: async () => {
      const { mode } = await host.requestDisplayMode({ mode: "fullscreen" });
      set({ context: { ...state.context, displayMode: mode } });
    },
  };
}
