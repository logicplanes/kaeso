import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { signedIn } from "~/test/fixtures";
import { modelsDev } from "~/test/fixtures/instance-account";
import { hoverReason, notifications } from "~/test/fixtures/org";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = "/settings/instance/model-sources";

describe("instance model sources page", () => {
  it("shows models.dev with the data in use, the last sync and the automatic sync", async () => {
    signedIn();
    on("GET", "instance/models-dev", modelsDev());
    await renderApp(PATH);
    expect(await screen.findByRole("heading", { name: "Model sources", level: 1 })).toBeInTheDocument();
    expect(await screen.findByText("412 models")).toBeInTheDocument();
    expect(screen.getByText("models.dev")).toBeInTheDocument();
    expect(screen.getByText("In use")).toBeInTheDocument();
    expect(screen.getByText("https://models.dev/api.json")).toBeInTheDocument();
    expect(screen.getByText(/^Fetched /)).toBeInTheDocument();
    expect(screen.getByText("Succeeded")).toBeInTheDocument();
    expect(screen.getByText("Every 1 h")).toBeInTheDocument();
    expect(screen.getByText("Gateway setting")).toBeInTheDocument();
  });

  it("shows the built-in snapshot, a failed sync and the interval in minutes", async () => {
    signedIn();
    on("GET", "instance/models-dev", modelsDev({ source: "embedded", model_count: 1, last_error: "connection refused", interval_seconds: 300 }));
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("1 model")).toBeInTheDocument();
    expect(screen.getByText(/^Snapshot, /)).toBeInTheDocument();
    expect(screen.getByText("Every 5 min")).toBeInTheDocument();
    await hoverReason(user, screen.getByText("Failed"), "connection refused");
  });

  it("says when no sync ran yet and automatic sync is off", async () => {
    signedIn();
    on("GET", "instance/models-dev", modelsDev({ last_attempt_at: null, sync_enabled: false }));
    await renderApp(PATH);
    expect(await screen.findByText("None yet")).toBeInTheDocument();
    expect(screen.getByText("Off")).toBeInTheDocument();
  });

  it("shows an interval in seconds", async () => {
    signedIn();
    on("GET", "instance/models-dev", modelsDev({ interval_seconds: 45 }));
    await renderApp(PATH);
    expect(await screen.findByText("Every 45 s")).toBeInTheDocument();
  });

  it("syncs on request and says how many models it has", async () => {
    signedIn();
    on("GET", "instance/models-dev", modelsDev());
    on("POST", "instance/models-dev/sync", modelsDev({ model_count: 420 }));
    const { user } = await renderApp(PATH);
    await screen.findByText("412 models");
    await user.click(screen.getByRole("button", { name: "Sync now" }));
    expect(await notifications().findByText("Synced: 420 models")).toBeInTheDocument();
    expect(requests("POST", "instance/models-dev/sync")).toHaveLength(1);
    await waitFor(() => expect(requests("GET", "instance/models-dev").length).toBeGreaterThan(1));
  });

  it("warns when models.dev couldn't be fetched", async () => {
    signedIn();
    on("GET", "instance/models-dev", modelsDev());
    on("POST", "instance/models-dev/sync", modelsDev({ last_error: "timeout" }));
    const { user } = await renderApp(PATH);
    await screen.findByText("412 models");
    await user.click(screen.getByRole("button", { name: "Sync now" }));
    expect(await notifications().findByText("models.dev couldn't be fetched; the current data stays in use")).toBeInTheDocument();
  });

  it("shows the API's message when the sync is refused", async () => {
    signedIn();
    on("GET", "instance/models-dev", modelsDev());
    on("POST", "instance/models-dev/sync", apiError(409, "conflict", "A sync is already running."));
    const { user } = await renderApp(PATH);
    await screen.findByText("412 models");
    await user.click(screen.getByRole("button", { name: "Sync now" }));
    expect((await notifications().findAllByText("A sync is already running.")).length).toBeGreaterThan(0);
  });

  it("says when the sources couldn't load and can't sync then", async () => {
    signedIn();
    on("GET", "instance/models-dev", apiError(403, "permission_denied", "Only instance admins can see this."));
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("Couldn't load these rows")).toBeInTheDocument();
    expect((await notifications().findAllByText("Only instance admins can see this.")).length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "Sync now" })).toBeDisabled();
    on("GET", "instance/models-dev", modelsDev());
    await user.click(within(screen.getByRole("table", { name: "Model sources" })).getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("412 models")).toBeInTheDocument();
  });
});
