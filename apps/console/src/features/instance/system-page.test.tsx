import { describe, expect, it } from "vitest";
import { screen, waitFor } from "@testing-library/react";
import { signedIn, whoAmI, user as account } from "~/test/fixtures";
import { system } from "~/test/fixtures/instance-account";
import { hoverReason } from "~/test/fixtures/org";
import { on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = "/settings/instance/system";

describe("instance system page", () => {
  it("shows the gateway, its services, the log buffer and storage", async () => {
    signedIn();
    on("GET", "instance/system", system());
    await renderApp(PATH);
    expect(await screen.findByRole("heading", { name: "System", level: 1 })).toBeInTheDocument();
    expect(await screen.findByText("Ready")).toBeInTheDocument();
    expect(screen.getByText("0.4.2")).toBeInTheDocument();
    expect(screen.getAllByText("Up")).toHaveLength(3);
    expect(screen.getByText("12")).toBeInTheDocument();
    expect(screen.getByText("None: every request was logged.")).toBeInTheDocument();
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

  it("checks again on request and says when it last checked", async () => {
    signedIn();
    on("GET", "instance/system", system());
    const { user } = await renderApp(PATH);
    await screen.findByText("Ready");
    const check = screen.getByRole("button", { name: "Check now" });
    await hoverReason(user, check, /Last checked .* · every 10 seconds/);
    const before = requests("GET", "instance/system").length;
    await user.click(check);
    await waitFor(() => expect(requests("GET", "instance/system").length).toBe(before + 1));
  });

  it("says how often it checks while the first check runs", async () => {
    signedIn();
    on("GET", "instance/system", () => new Promise(() => {}));
    const { user } = await renderApp(PATH);
    await hoverReason(user, await screen.findByRole("button", { name: "Check now" }), "Checked every 10 seconds");
    expect(screen.queryByText("Ready")).not.toBeInTheDocument();
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

  it("opens on the users page", async () => {
    signedIn();
    on("GET", "instance/users", { data: [], has_more: false, next_cursor: null });
    await renderApp("/settings/instance");
    expect(await screen.findByRole("heading", { name: "Users", level: 1 })).toBeInTheDocument();
    expect(window.location.pathname).toBe("/settings/instance/users");
  });
});
