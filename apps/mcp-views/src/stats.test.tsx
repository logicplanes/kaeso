import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App } from "@modelcontextprotocol/ext-apps";
import { AppBridge } from "@modelcontextprotocol/ext-apps/app-bridge";
import type { McpUiHostContext } from "@modelcontextprotocol/ext-apps";
import { InMemoryTransport } from "@modelcontextprotocol/client";
import type { CallToolRequest, CallToolResult } from "@modelcontextprotocol/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { formatDuration } from "@logicplanes/kaeso-ui/format";
import { createStore } from "./store";
import { StatsView, formatValue, money } from "./stats";
import { setLocale } from "./i18n";
import { ok, refused, stats } from "./test/fixtures";

type Calls = CallToolRequest["params"][];

// The official host side of MCP Apps, so the view is tested against the protocol as the SDK speaks it.
async function host(context: McpUiHostContext = {}, answer: (p: CallToolRequest["params"]) => CallToolResult = () => ok(stats())) {
  const [viewSide, hostSide] = InMemoryTransport.createLinkedPair();
  const bridge = new AppBridge(
    null,
    { name: "test-host", version: "1.0.0" },
    { openLinks: {}, serverTools: {} },
    {
      hostContext: { theme: "light", locale: "en-US", availableDisplayModes: ["inline", "fullscreen"], displayMode: "inline", ...context },
    },
  );
  const calls: Calls = [];
  const links: string[] = [];
  const modes: string[] = [];
  bridge.oncalltool = async (params) => {
    calls.push(params);
    return answer(params);
  };
  bridge.onopenlink = async ({ url }) => {
    links.push(url);
    return {};
  };
  bridge.onrequestdisplaymode = async ({ mode }) => {
    modes.push(mode);
    return { mode };
  };
  bridge.onupdatemodelcontext = async () => ({});
  const initialized = new Promise<void>((resolve) => (bridge.oninitialized = () => resolve()));
  await bridge.connect(hostSide);
  const app = new App({ name: "Kaeso usage stats", version: "test" }, { availableDisplayModes: ["inline", "fullscreen"] }, { autoResize: false });
  const store = createStore(app);
  render(<StatsView store={store} />);
  await act(async () => {
    await app.connect(viewSide);
    store.connected();
    await initialized;
  });
  return { bridge, calls, links, modes, store };
}

afterEach(() => setLocale("en"));

describe("the usage view", () => {
  it("shows its shape while the tool runs, then the totals, the chart and the breakdown", async () => {
    const { bridge } = await host();
    expect(screen.getByRole("heading", { name: "Usage" })).toBeVisible();
    expect(screen.getByText("Last 7 days")).toBeVisible();
    await act(async () => {
      await bridge.sendToolInput({ arguments: { metric: "cost", range: "7d", group_by: "model", agent_model: "claude" } });
      await bridge.sendToolResult(ok(stats()));
    });
    expect(await screen.findByText(/Acme · Last 7 days/)).toBeVisible();
    const totals = screen.getByLabelText("Totals");
    expect(within(totals).getByText("1,234")).toBeVisible();
    expect(within(totals).getByText("$12.50")).toBeVisible();
    expect(within(totals).getByText(formatDuration(1800))).toBeVisible();
    const breakdown = screen.getByRole("list", { name: "Breakdown by Model" });
    expect(within(breakdown).getByText("openai/gpt-5")).toBeVisible();
    expect(within(breakdown).getByText("$8.00")).toBeVisible();
    expect(screen.getAllByLabelText("Cost over time")[0]).toBeInTheDocument();
  });

  it("asks Kaeso again through the host when a control changes, with the agent's own arguments", async () => {
    const { bridge, calls } = await host({}, (p) =>
      ok(stats({ range: (p.arguments?.range as "24h") ?? "7d", step: "hour", series: [{ start: "2026-11-10T12:00:00Z", value: "1" }] })),
    );
    await act(async () => {
      await bridge.sendToolInput({ arguments: { metric: "cost", agent_model: "claude", org: "org_1" } });
      await bridge.sendToolResult(ok(stats()));
    });
    await userEvent.click(screen.getByRole("radio", { name: "24h" }));
    await waitFor(() => expect(calls).toHaveLength(1));
    expect(calls[0]).toMatchObject({
      name: "show_usage_stats",
      arguments: { metric: "cost", agent_model: "claude", org: "org_1", range: "24h", group_by: "model" },
    });
    expect(await screen.findByText(/Last 24 hours/)).toBeVisible();
  });

  it("shows the approve link a missing right names and opens it through the host", async () => {
    const { bridge, links } = await host();
    await act(async () => {
      await bridge.sendToolInput({ arguments: {} });
      await bridge.sendToolResult(refused);
    });
    expect(await screen.findByText("Kaeso needs your approval")).toBeVisible();
    expect(document.querySelector("code")).toHaveTextContent("usage.org.view");
    expect(screen.queryByText(/`/)).toBeNull();
    expect(screen.queryByRole("group", { name: "Totals" })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Approve in Kaeso" }));
    expect(links).toEqual(["https://kaeso.example.com/settings/account/agents/agt_1?add=usage.org.view"]);
  });

  it("opens the console and the side panel only where the host offers them", async () => {
    const { bridge, links, modes } = await host();
    await act(async () => {
      await bridge.sendToolInput({ arguments: {} });
      await bridge.sendToolResult(ok(stats()));
    });
    await userEvent.click(await screen.findByRole("button", { name: "Open in Kaeso" }));
    expect(links).toEqual(["https://kaeso.example.com/orgs/org_1/analytics"]);
    await userEvent.click(screen.getByRole("button", { name: "Open beside the chat" }));
    expect(modes).toEqual(["fullscreen"]);
    await waitFor(() => expect(screen.queryByRole("button", { name: "Open beside the chat" })).toBeNull());
  });

  it("writes every value from Kaeso as text, never as markup", async () => {
    const { bridge } = await host();
    const label = '<img src=x onerror="window.hacked=1">';
    await act(async () => {
      await bridge.sendToolInput({ arguments: {} });
      await bridge.sendToolResult(
        ok(stats({ scope: { org: { id: "org_1", name: label }, project: null }, groups: [{ key: "k", label, value: "1", requests: 1 }] })),
      );
    });
    expect(await screen.findAllByText(new RegExp(label.slice(0, 8)))).not.toHaveLength(0);
    expect(document.querySelector("img[src=x]")).toBeNull();
  });

  it("follows the host's language and theme", async () => {
    const { bridge } = await host({ locale: "de-DE", theme: "dark" });
    await act(async () => {
      await bridge.sendToolInput({ arguments: {} });
      await bridge.sendToolResult(ok(stats({ scope: { org: { id: "org_1", name: "Acme" }, project: { id: "proj_1", name: "Shop" } } })));
    });
    expect(await screen.findByRole("heading", { name: "Nutzung" })).toBeVisible();
    expect(screen.getByText(/Shop in Acme · Letzte 7 Tage/)).toBeVisible();
    await waitFor(() => expect(document.documentElement).toHaveAttribute("data-theme", "dark"));
    act(() =>
      bridge.setHostContext({
        theme: "light",
        locale: "zh-CN",
        availableDisplayModes: ["inline"],
        displayMode: "inline",
        safeAreaInsets: { top: 0, right: 0, bottom: 24, left: 0 },
      }),
    );
    expect(await screen.findByRole("heading", { name: "用量" })).toBeVisible();
    await waitFor(() => expect(document.documentElement.style.getPropertyValue("--safe-bottom")).toBe("24px"));
    expect(screen.queryByRole("button", { name: "在对话旁打开" })).toBeNull();
  });

  it("says when the call was cancelled or the host failed", async () => {
    const { bridge, store } = await host({}, () => {
      throw new Error("The host refused the call.");
    });
    await act(async () => {
      await bridge.sendToolCancelled({ reason: "user" });
    });
    expect(await screen.findByText("The call was cancelled.")).toBeVisible();
    await act(() => store.change({ metric: "errors" }));
    expect(await screen.findByText(/The host refused the call/)).toBeVisible();
  });
});

describe("values", () => {
  it("writes money, durations, tokens and hidden values the way the console does", () => {
    expect(money("0.001")).toBe("<$0.01");
    expect(money("12.5")).toBe("$12.50");
    expect(formatValue("cost", null)).toBe("—");
    expect(formatValue("latency", 250)).toBe(formatDuration(250));
    expect(formatValue("tokens", 2_500_000)).toBe("2.5M");
    expect(formatValue("requests", "1234")).toBe("1,234");
  });
});

vi.stubGlobal(
  "ResizeObserver",
  class {
    observe() {}
    unobserve() {}
    disconnect() {}
  },
);
