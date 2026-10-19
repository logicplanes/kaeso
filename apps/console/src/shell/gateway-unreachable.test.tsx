import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, onTestFinished, vi } from "vitest";
import { ORG, signedIn } from "~/test/fixtures";
import { notifications } from "~/test/fixtures/org";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { queryClient } from "~/query";
import { sessionQuery } from "~/auth/session-queries";

const RETRYING = "Can't reach the gateway; retrying";

describe("when the gateway can't be reached at start", () => {
  it("says so, keeps retrying on request and opens the console once it answers", async () => {
    on("GET", "auth/session", apiError(400, "unavailable", "The gateway is starting."));
    const { user } = await renderApp(`/orgs/${ORG}/projects`);
    expect(await screen.findByRole("heading", { name: "Kaeso isn't answering" })).toBeInTheDocument();
    expect(screen.getByText("The console reaches the gateway on this domain; check that it's running.")).toBeInTheDocument();
    expect(await notifications().findByText(RETRYING)).toBeInTheDocument();
    expect(notifications().queryByText("The gateway is starting.")).not.toBeInTheDocument();
    const before = requests("GET", "auth/session").length;
    await user.click(within(screen.getByRole("main")).getByRole("button", { name: "Retry now" }));
    await waitFor(() => expect(requests("GET", "auth/session").length).toBe(before + 1));
    expect(screen.getByRole("heading", { name: "Kaeso isn't answering" })).toBeInTheDocument();
    signedIn();
    await user.click(notifications().getByRole("button", { name: "Retry now", hidden: true }));
    expect(await screen.findByRole("navigation", { name: "Console" })).toBeInTheDocument();
    await waitFor(() => expect(notifications().queryByText(RETRYING)).not.toBeInTheDocument());
  });

  it("tries again on its own every few seconds", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"], shouldAdvanceTime: true });
    onTestFinished(() => void vi.useRealTimers());
    on("GET", "auth/session", apiError(400, "unavailable", "The gateway is starting."));
    await renderApp(`/orgs/${ORG}/projects`);
    expect(await screen.findByRole("heading", { name: "Kaeso isn't answering" })).toBeInTheDocument();
    signedIn();
    await vi.advanceTimersByTimeAsync(5000);
    expect(await screen.findByRole("navigation", { name: "Console" })).toBeInTheDocument();
    await waitFor(() => expect(notifications().queryByText(RETRYING)).not.toBeInTheDocument());
  });

  it("shows the API's message when the session fails for another reason", async () => {
    on("GET", "auth/session", apiError(400, "invalid_request", "The session cookie is malformed."));
    const { user } = await renderApp(`/orgs/${ORG}/projects`);
    expect(await screen.findByRole("heading", { name: "Kaeso isn't answering" })).toBeInTheDocument();
    expect(await notifications().findAllByText("The session cookie is malformed.")).not.toHaveLength(0);
    expect(notifications().queryByText(RETRYING)).not.toBeInTheDocument();
    signedIn();
    await user.click(within(screen.getByRole("main")).getByRole("button", { name: "Retry now" }));
    expect(await screen.findByRole("navigation", { name: "Console" })).toBeInTheDocument();
  });
});

describe("when the gateway stops answering later", () => {
  it("warns until the session answers again", async () => {
    signedIn();
    const { user } = await renderApp(`/orgs/${ORG}/projects`);
    await screen.findByRole("navigation", { name: "Console" });
    on("GET", "auth/session", apiError(400, "unavailable", "The gateway is restarting."));
    queryClient
      .getQueryCache()
      .find({ queryKey: sessionQuery.queryKey })
      ?.setState({ dataUpdatedAt: Date.now() - 60_000 });
    window.dispatchEvent(new Event("visibilitychange"));
    expect(await notifications().findByText(RETRYING)).toBeInTheDocument();
    expect(notifications().queryByText("The gateway is restarting.")).not.toBeInTheDocument();
    signedIn();
    await user.click(notifications().getByRole("button", { name: "Retry now", hidden: true }));
    await waitFor(() => expect(notifications().queryByText(RETRYING)).not.toBeInTheDocument());
    expect(screen.getByRole("navigation", { name: "Console" })).toBeInTheDocument();
  });
});
