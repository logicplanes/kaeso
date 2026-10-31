import { describe, expect, it, onTestFinished, vi } from "vitest";
import { act, screen, waitFor } from "@testing-library/react";
import { signedIn, whoAmI, user as account } from "~/test/fixtures";
import { instanceSettings, system } from "~/test/fixtures/instance-account";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = "/settings/instance/system";

describe("instance system page", () => {
  it("shows the gateway, its services, the log buffer and storage", async () => {
    signedIn();
    on("GET", "instance/system", system());
    await renderApp(PATH);
    expect(await screen.findByRole("heading", { name: "System", level: 1 })).toBeInTheDocument();
    expect(await screen.findByText("Ready")).toBeInTheDocument();
    expect(screen.getByText("40.4.2")).toBeInTheDocument();
    expect(screen.getAllByText("Up")).toHaveLength(3);
    expect(screen.getByText("12")).toBeInTheDocument();
    expect(screen.getByText("Every request since the start was logged.")).toBeInTheDocument();
    expect(screen.getByText("2.0 MB")).toBeInTheDocument();
    expect(screen.getByText("10.0 MB")).toBeInTheDocument();
    expect(screen.getByText("48,210")).toBeInTheDocument();
  });

  it("says when a service is down and requests were dropped", async () => {
    signedIn();
    on(
      "GET",
      "instance/system",
      system({
        status: "degraded",
        clickhouse: "unreachable",
        log_buffer: { rows: 900, dropped: 37 },
        storage: { postgres_bytes: 1024, clickhouse_bytes_on_disk: null, request_log_rows: null },
      }),
    );
    await renderApp(PATH);
    expect(await screen.findByText("Degraded")).toBeInTheDocument();
    expect(screen.getByText("Down")).toBeInTheDocument();
    expect(screen.getAllByText("Up")).toHaveLength(2);
    expect(screen.getByText("37")).toBeInTheDocument();
    expect(screen.getByText("The buffer was full: these requests have no log entry.")).toBeInTheDocument();
    expect(screen.getAllByText("Unknown")).toHaveLength(2);
  });

  it("says when the gateway isn't ready", async () => {
    signedIn();
    on("GET", "instance/system", system({ status: "not_ready" }));
    await renderApp(PATH);
    expect(await screen.findByText("Not ready")).toBeInTheDocument();
  });

  it("shows an unknown status by its name", async () => {
    signedIn();
    on("GET", "instance/system", system({ status: "starting" as never }));
    await renderApp(PATH);
    expect(await screen.findByText("starting")).toBeInTheDocument();
  });

  it("refreshes by itself every 10 seconds, without a button", async () => {
    signedIn();
    on("GET", "instance/system", system());
    await renderApp(PATH);
    await screen.findByText("Ready");
    expect(screen.getByText("Live state of the gateway and its services, refreshed every 10 seconds.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Check now" })).not.toBeInTheDocument();
  });

  it("says when the system status couldn't load", async () => {
    signedIn();
    on("GET", "instance/system", apiError(429, "rate_limited", "Too many requests; try again in a minute."));
    await renderApp(PATH);
    expect(await screen.findByText("The system status couldn't load")).toBeInTheDocument();
  });

  it("keeps saying it couldn't load while the next refresh tries again", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "setTimeout", "clearTimeout"], shouldAdvanceTime: true });
    onTestFinished(() => void vi.useRealTimers());
    signedIn();
    let calls = 0;
    on("GET", "instance/system", () => (++calls === 1 ? apiError(429, "rate_limited", "Too many requests; try again in a minute.") : new Promise(() => {})));
    await renderApp(PATH);
    expect(await screen.findByText("The system status couldn't load")).toBeInTheDocument();
    await act(() => vi.advanceTimersByTimeAsync(10_000));
    await waitFor(() => expect(calls).toBeGreaterThan(1));
    expect(screen.getByText("The system status couldn't load")).toBeInTheDocument();
  });
});

describe("the instance area", () => {
  it("is only for instance admins", async () => {
    signedIn({ session: whoAmI({ user: account({ is_instance_admin: false }) }) });
    await renderApp(PATH);
    expect(await screen.findByText("For instance admins")).toBeInTheDocument();
    expect(screen.getByText("These pages manage the whole install; an instance admin can make you one.")).toBeInTheDocument();
    expect(requests("GET", "instance/system")).toHaveLength(0);
  });

  it("opens on the general page", async () => {
    signedIn();
    on("GET", "instance/settings", instanceSettings());
    await renderApp("/settings/instance");
    expect(await screen.findByRole("heading", { name: "General", level: 1 })).toBeInTheDocument();
    expect(window.location.pathname).toBe("/settings/instance/general");
  });

  it("shows ClickHouse as off and leaves out the request log when the install keeps none", async () => {
    signedIn();
    on("GET", "instance/system", system({ clickhouse: "off", storage: { postgres_bytes: 2_097_152, clickhouse_bytes_on_disk: null, request_log_rows: null } }));
    await renderApp(PATH);
    expect(await screen.findByText("Off")).toBeInTheDocument();
    expect(screen.getByText("Not set up, so request logs are off.")).toBeInTheDocument();
    expect(screen.getAllByText("Up")).toHaveLength(2);
    expect(screen.queryByText("Down")).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Request log buffer" })).not.toBeInTheDocument();
    expect(screen.queryByText("Request log entries")).not.toBeInTheDocument();
    expect(screen.queryByText("Unknown")).not.toBeInTheDocument();
  });
});
