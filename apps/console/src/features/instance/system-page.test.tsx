import { describe, expect, it, onTestFinished, vi } from "vitest";
import { act, screen, waitFor } from "@testing-library/react";
import { signedIn, whoAmI, user as account } from "~/test/fixtures";
import { instanceSettings, system } from "~/test/fixtures/instance-account";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { hoverReason, notifications } from "~/test/fixtures/org";

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

  it("shows the model data from KPMR and how it stays current", async () => {
    signedIn();
    on("GET", "instance/system", system());
    await renderApp(PATH);
    expect(await screen.findByRole("heading", { name: "Model data", level: 2 })).toBeInTheDocument();
    expect(screen.getByText("Model names, limits and prices from KPMR, Kaeso's model registry.")).toBeInTheDocument();
    expect(screen.getByText("1,060 models")).toBeInTheDocument();
    expect(screen.getByText(/^Fetched /)).toBeInTheDocument();
    expect(screen.getByText("https://kpmr.kaeso.app/v1/catalogue")).toBeInTheDocument();
    expect(screen.getByText("KPMR")).toBeInTheDocument();
    expect(screen.getByText("On")).toBeInTheDocument();
    expect(screen.getByText("Succeeded")).toBeInTheDocument();
  });

  it("says when the built-in snapshot is in use, sync is off or the last sync failed", async () => {
    signedIn();
    const base = system().model_data;
    on("GET", "instance/system", system({ model_data: { ...base, built_in: true, sync_enabled: false, last_error: "the catalogue is not signed" } }));
    const { user } = await renderApp(PATH);
    expect(await screen.findByText(/^Snapshot, /)).toBeInTheDocument();
    expect(screen.getByText("Off")).toBeInTheDocument();
    await user.hover(screen.getByText("Failed"));
    expect(await screen.findByRole("tooltip")).toHaveTextContent("the catalogue is not signed");
  });

  it("fetches the model data now and refreshes the card", async () => {
    signedIn();
    on("GET", "instance/system", system());
    let answer: unknown = { status: "applied", price_changes: 3, delisted: 0, relisted: 0 };
    on("POST", "instance/kpmr/sync", () => answer);
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Sync now" }));
    expect(await notifications().findByText("New model data applied: 3 price changes")).toBeInTheDocument();
    expect(requests("POST", "instance/kpmr/sync")).toHaveLength(1);
    await waitFor(() => expect(requests("GET", "instance/system").length).toBeGreaterThan(1));
    answer = { status: "unchanged", price_changes: 0, delisted: 0, relisted: 0 };
    await user.click(screen.getByRole("button", { name: "Sync now" }));
    expect(await notifications().findByText("The model data is current")).toBeInTheDocument();
  });

  it("refreshes the card when fetching the model data failed, and offers no sync while it is off", async () => {
    signedIn();
    on("GET", "instance/system", system());
    on("POST", "instance/kpmr/sync", apiError(503, "unavailable", "Kaeso could not fetch the model data; the System page says why."));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Sync now" }));
    expect(await notifications().findByText("Kaeso could not fetch the model data; the System page says why.")).toBeInTheDocument();
    await waitFor(() => expect(requests("GET", "instance/system").length).toBeGreaterThan(1));
  });

  it("explains why it can't sync while sync is off", async () => {
    signedIn();
    const base = system().model_data;
    on("GET", "instance/system", system({ model_data: { ...base, sync_enabled: false } }));
    const { user } = await renderApp(PATH);
    await hoverReason(user, await screen.findByRole("button", { name: "Sync now" }), "Sync is off; set KAESO_KPMR_SYNC=true to fetch model data");
  });

  it("says when the gateway hasn't asked KPMR yet", async () => {
    signedIn();
    const base = system().model_data;
    on("GET", "instance/system", system({ model_data: { ...base, last_attempt_at: null } }));
    await renderApp(PATH);
    expect(await screen.findByText("None yet")).toBeInTheDocument();
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
