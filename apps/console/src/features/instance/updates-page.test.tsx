import { describe, expect, it, vi } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { signedIn } from "~/test/fixtures";
import { available, release, updateJob, updateSettings, updates } from "~/test/fixtures/instance-account";
import { hoverReason } from "~/test/fixtures/org";
import { apiError, on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import type { UpdatesView } from "~/api/generated/types";

const PATH = "/settings/instance/updates";

async function choose(user: Awaited<ReturnType<typeof renderApp>>["user"], select: string, option: string) {
  await user.click(screen.getByRole("combobox", { name: select }));
  await user.click(await screen.findByRole("option", { name: new RegExp(`^${option}`) }));
}

function history(jobs: ReturnType<typeof updateJob>[] = []) {
  on("GET", "instance/updates/jobs", page(jobs));
}

describe("instance updates page", () => {
  it("loads older updates on request", async () => {
    signedIn();
    on("GET", "instance/updates", updates());
    on("GET", "instance/updates/jobs", ({ query }: { query: URLSearchParams }) =>
      query.get("after") === "cur_2"
        ? page([
            updateJob({
              id: "upd_01k7a3f8m2xq4v9c5r6t8w0yz3",
              from: "40.4.0",
              to: "40.4.1",
              state: "succeeded",
              step: "online",
              finished_at: "2026-09-01T12:01:00Z",
            }),
          ])
        : page([updateJob({ state: "succeeded", step: "online", finished_at: "2026-10-15T12:01:00Z" })], "cur_2"),
    );
    const { user } = await renderApp(PATH);
    const table = await screen.findByRole("table", { name: "History" });
    expect(await within(table).findByText("40.4.2 to 40.4.3")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Load more" }));
    expect(await within(table).findByText("40.4.0 to 40.4.1")).toBeInTheDocument();
  });

  it("says the install is up to date and when it last looked", async () => {
    signedIn();
    on("GET", "instance/updates", updates());
    history();
    await renderApp(PATH);
    expect(await screen.findByRole("heading", { name: "Updates", level: 1 })).toBeInTheDocument();
    expect(await screen.findByText("Up to date")).toBeInTheDocument();
    expect(screen.getByText("Kaeso 40.4.2")).toBeInTheDocument();
    expect(screen.getByText(/^Checked .*; Kaeso looks twice a day\.$/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Check now" })).not.toBeInTheDocument();
    expect(await screen.findByText("No updates yet")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Update to/ })).not.toBeInTheDocument();
  });

  it("shows a new release with its notes and updates after a confirmation", async () => {
    signedIn();
    on("GET", "instance/updates", available());
    history();
    let polls = 0;
    on("POST", "instance/updates/apply", () => updateJob());
    on("GET", "instance/updates/jobs/:update", () => {
      polls += 1;
      return polls < 2 ? updateJob({ step: "migrating" }) : updateJob({ state: "succeeded", step: "online", finished_at: "2026-10-15T12:01:00Z" });
    });
    const { user } = await renderApp(PATH);
    expect(await screen.findByRole("button", { name: "Update to 40.4.3" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Kaeso 40.4.3" })).toBeInTheDocument();
    const notes = screen.getByRole("region", { name: "Release notes of 40.4.3" });
    expect(within(notes).getByText("Streams keep their last chunk when a provider closes early.")).toBeInTheDocument();
    expect(within(notes).getByRole("link", { name: /On GitHub/ })).toHaveAttribute("href", "https://github.com/logicplanes/kaeso/releases/tag/v40.4.3");

    await user.click(screen.getByRole("button", { name: "Update to 40.4.3" }));
    const confirm = await screen.findByRole("dialog", { name: "Update to Kaeso 40.4.3?" });
    expect(within(confirm).getByText(/Back up PostgreSQL and ClickHouse first/)).toBeInTheDocument();
    expect(within(confirm).getByText(/signed by Kaeso's release workflow/)).toBeInTheDocument();
    await user.click(within(confirm).getByRole("button", { name: "Update now" }));
    await waitFor(() => expect(requests("POST", "instance/updates/apply").at(-1)?.body).toEqual({ version: "40.4.3" }));

    const progress = await screen.findByRole("dialog", { name: "Updating to Kaeso 40.4.3" });
    const steps = within(progress).getByRole("group", { name: "Steps" });
    await waitFor(() => expect(within(steps).getAllByText("Done")).toHaveLength(3));
    expect(within(steps).getByText("In progress")).toBeInTheDocument();
    expect(within(steps).getByText("Waiting")).toBeInTheDocument();
    expect(await screen.findByRole("dialog", { name: "Kaeso 40.4.3 is running" }, { timeout: 5_000 })).toBeInTheDocument();
    await waitFor(() => expect(requests("GET", "instance/updates/jobs").length).toBeGreaterThan(1));
    const reload = vi.spyOn(window.location, "reload").mockImplementation(() => undefined);
    await user.click(screen.getByRole("button", { name: "Reload the console" }));
    expect(reload).toHaveBeenCalledOnce();
    reload.mockRestore();
  });

  it("cancels a confirmation, and sends it with the keyboard", async () => {
    signedIn();
    on("GET", "instance/updates", available());
    history();
    on("POST", "instance/updates/apply", () => updateJob());
    on("GET", "instance/updates/jobs/:update", () => updateJob({ step: "verifying" }));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Update to 40.4.3" }));
    await user.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(requests("POST", "instance/updates/apply")).toHaveLength(0);
    await user.click(screen.getByRole("button", { name: "Update to 40.4.3" }));
    await screen.findByRole("dialog", { name: "Update to Kaeso 40.4.3?" });
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Update to 40.4.3" }));
    await screen.findByRole("dialog", { name: "Update to Kaeso 40.4.3?" });
    await user.keyboard("{Control>}{Enter}{/Control}");
    await waitFor(() => expect(requests("POST", "instance/updates/apply")).toHaveLength(1));
    expect(await screen.findByRole("dialog", { name: "Updating to Kaeso 40.4.3" })).toBeInTheDocument();
  });

  it("offers to load the history again when it fails", async () => {
    signedIn();
    on("GET", "instance/updates", updates());
    let failing = true;
    on("GET", "instance/updates/jobs", () => (failing ? apiError(429, "rate_limited", "Too many requests; try again in a minute.") : page([])));
    const { user } = await renderApp(PATH);
    const again = await screen.findByRole("button", { name: "Try again" });
    failing = false;
    await user.click(again);
    expect(await screen.findByText("No updates yet")).toBeInTheDocument();
  });

  it("explains a failed update and what runs now", async () => {
    signedIn();
    on("GET", "instance/updates", available({ job: updateJob(), can_apply: false }));
    history();
    on(
      "GET",
      "instance/updates/jobs/:update",
      updateJob({
        state: "rolled_back",
        step: "migrating",
        reason: "unhealthy",
        detail: "the new gateway reported itself unhealthy",
        finished_at: "2026-10-15T12:02:00Z",
      }),
    );
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("Updating to Kaeso 40.4.3")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Show progress" }));
    const dialog = await screen.findByRole("dialog");
    expect(await within(dialog).findByText("The new version didn't start.")).toBeInTheDocument();
    expect(within(dialog).getByText("Kaeso 40.4.2 runs again, unchanged.")).toBeInTheDocument();
    expect(within(dialog).getByText("the new gateway reported itself unhealthy")).toBeInTheDocument();
    expect(within(dialog).getByText("Stopped")).toBeInTheDocument();
    await user.click(within(dialog).getAllByRole("button", { name: "Close" }).at(-1) as HTMLElement);
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("says when Kaeso is away during the switch and when the previous version doesn't come back", async () => {
    signedIn();
    on("GET", "instance/updates", available({ job: updateJob(), can_apply: false }));
    history();
    let calls = 0;
    on("GET", "instance/updates/jobs/:update", () => {
      calls += 1;
      return calls === 1
        ? apiError(503, "unavailable", "Kaeso is being updated; try again in a minute.")
        : updateJob({ state: "rollback_failed", step: "migrating", finished_at: "2026-10-15T12:02:00Z" });
    });
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Show progress" }));
    expect(await screen.findByText("Kaeso is switching over; this window reconnects by itself.")).toBeInTheDocument();
    expect(await screen.findByText("The update didn't finish.", {}, { timeout: 5_000 })).toBeInTheDocument();
    expect(screen.getByText("The previous version didn't start again; follow the upgrade guide to recover.")).toBeInTheDocument();
  });

  it("hands a gateway that may have migrated the database to a person", async () => {
    signedIn();
    on("GET", "instance/updates", available({ job: updateJob(), can_apply: false }));
    history();
    on(
      "GET",
      "instance/updates/jobs/:update",
      updateJob({
        state: "needs_attention",
        step: "migrating",
        reason: "migrated",
        detail: "the new gateway was not healthy within 600 seconds",
        finished_at: "2026-10-15T12:02:00Z",
      }),
    );
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Show progress" }));
    const dialog = await screen.findByRole("dialog");
    expect(await within(dialog).findByText("Needs attention")).toBeInTheDocument();
    expect(
      within(dialog).getByText("The new gateway may have migrated the database, so Kaeso 40.4.2 wasn't started again; follow the upgrade guide."),
    ).toBeInTheDocument();
    expect(within(dialog).queryByText("Kaeso 40.4.2 runs again, unchanged.")).not.toBeInTheDocument();
  });

  it("names no migration when something else needs attention", async () => {
    signedIn();
    on("GET", "instance/updates", available({ job: updateJob(), can_apply: false }));
    history();
    on(
      "GET",
      "instance/updates/jobs/:update",
      updateJob({
        state: "needs_attention",
        step: "online",
        reason: "state_unwritable",
        detail: "the maintenance flag could not be removed",
        finished_at: "2026-10-15T12:02:00Z",
      }),
    );
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Show progress" }));
    const dialog = await screen.findByRole("dialog");
    expect(await within(dialog).findByText("Kaeso needs someone to look at its containers; the detail below says what happened.")).toBeInTheDocument();
    expect(within(dialog).queryByText(/may have migrated the database/)).not.toBeInTheDocument();
  });

  it("says what is left when the new version runs but compose.yaml couldn't be written", async () => {
    signedIn();
    on("GET", "instance/updates", available({ job: updateJob(), can_apply: false }));
    history();
    on(
      "GET",
      "instance/updates/jobs/:update",
      updateJob({
        state: "succeeded",
        step: "online",
        reason: "compose_unwritable",
        detail: "40.4.3 runs, but compose.yaml cannot be written: read-only file system",
        finished_at: "2026-10-15T12:02:00Z",
      }),
    );
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Show progress" }));
    const dialog = await screen.findByRole("dialog");
    expect(await within(dialog).findByText("Kaeso 40.4.3 is running")).toBeInTheDocument();
    expect(within(dialog).getByText("One step is left")).toBeInTheDocument();
    expect(within(dialog).getByText("Set the Kaeso images in compose.yaml to 40.4.3 by hand.")).toBeInTheDocument();
  });

  it("sends a major release to the upgrade guide", async () => {
    signedIn();
    on(
      "GET",
      "instance/updates",
      available({
        manual: "major",
        can_apply: false,
        latest: release({ version: "41.0.0", major: true }),
        releases: [release({ version: "41.0.0", major: true })],
      }),
    );
    history();
    await renderApp(PATH);
    expect(await screen.findByText("Update by hand")).toBeInTheDocument();
    expect(screen.getByText("Kaeso 41.0.0 is a new major version: update by hand with the upgrade guide.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /Open the upgrade guide/ })).toHaveAttribute("href", "https://docs.kaeso.app/self-hosting/upgrades");
    expect(screen.getByText("Major version")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Update to/ })).not.toBeInTheDocument();
  });

  it("names manual steps on the way", async () => {
    signedIn();
    on("GET", "instance/updates", available({ manual: "manual_steps", can_apply: false }));
    history();
    await renderApp(PATH);
    expect(await screen.findByText("A release on the way asks for steps by hand: follow the upgrade guide.")).toBeInTheDocument();
  });

  it("names releases without a manifest", async () => {
    signedIn();
    on("GET", "instance/updates", available({ manual: "not_prepared", can_apply: false, releases: [release({ notes: " ", prerelease: true })] }));
    history();
    await renderApp(PATH);
    expect(await screen.findByText("This release can't be applied from here: update by hand with the upgrade guide.")).toBeInTheDocument();
    expect(screen.getByText("This release has no notes.")).toBeInTheDocument();
    expect(screen.getByText("Pre-release")).toBeInTheDocument();
  });

  it.each([
    [{ updater: { state: "not_installed", version: null } }, "The updater isn't installed, so this install updates by hand."],
    [{ updater: { state: "unreachable", version: null } }, "The updater doesn't answer; check that its container runs."],
    [{ updater: { state: "outdated", version: "40.4.0" } }, "The updater is too old for this release; update by hand once."],
  ] as const)("shows the commands to update by hand when the updater can't (%#)", async (change, reason) => {
    signedIn();
    on("GET", "instance/updates", available({ can_apply: false, ...change }));
    history();
    await renderApp(PATH);
    expect(await screen.findByText(reason)).toBeInTheDocument();
    expect(screen.getByText(/releases\/download\/v40\.4\.3\/compose\.yaml/)).toBeInTheDocument();
    expect(document.body.textContent).toContain("docker compose up -d --wait");
    for (const link of screen.getAllByRole("link", { name: /Set up the updater/ }))
      expect(link).toHaveAttribute("href", "https://docs.kaeso.app/self-hosting/upgrades#the-updater");
  });

  it("says when the last check failed", async () => {
    signedIn();
    on(
      "GET",
      "instance/updates",
      updates({
        state: "unknown",
        check: {
          last_checked_at: "2026-10-15T11:00:00Z",
          last_success_at: null,
          next_check_at: null,
          error: "api.github.com cannot be reached (no connection)",
        },
      }),
    );
    history();
    const { user } = await renderApp(PATH);
    const failed = await screen.findByText(/^The check .* failed; Kaeso tries again later\.$/);
    expect(screen.getByText("Not checked yet")).toBeInTheDocument();
    await hoverReason(user, failed, "api.github.com can't be reached: no connection");
  });

  it("says when it never looked and that checks are off", async () => {
    signedIn();
    on(
      "GET",
      "instance/updates",
      updates({ settings: updateSettings({ check: false }), check: { last_checked_at: null, last_success_at: null, next_check_at: null, error: null } }),
    );
    history();
    await renderApp(PATH);
    expect(await screen.findByText("Checking is off in the gateway's configuration.")).toBeInTheDocument();
  });

  it("changes the settings and says what changed", async () => {
    signedIn();
    let current = updateSettings();
    on("GET", "instance/updates", () => updates({ settings: current }));
    history();
    on("PATCH", "instance/updates/settings", ({ body }: { body: Partial<UpdatesView["settings"]> }) => {
      current = { ...current, ...body };
      return updates({ settings: current });
    });
    const { user } = await renderApp(PATH);
    await choose(user, "Releases", "Stable and pre-releases");
    expect(await screen.findByText("Channel saved")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole("combobox", { name: "Releases" })).toHaveTextContent("Stable and pre-releases"));
    await choose(user, "Releases", "Stable releases");
    await waitFor(() => expect(requests("PATCH", "instance/updates/settings")).toHaveLength(2));
    expect(requests("PATCH", "instance/updates/settings").map((r) => r.body)).toEqual([{ channel: "prerelease" }, { channel: "stable" }]);
    expect(screen.queryByRole("switch")).not.toBeInTheDocument();
    expect(screen.queryByText("Updater")).not.toBeInTheDocument();
  });

  it("says when the updater doesn't answer", async () => {
    signedIn();
    on("GET", "instance/updates", available({ can_apply: false, updater: { state: "unreachable", version: null } }));
    history();
    await renderApp(PATH);
    expect(await screen.findByText("The updater doesn't answer; check that its container runs.")).toBeInTheDocument();
    expect(screen.getByText("40.4.3 available")).toBeInTheDocument();
  });

  it("needs the updater before updates can install automatically", async () => {
    signedIn();
    on("GET", "instance/updates", updates({ updater: { state: "not_installed", version: null } }));
    history();
    const { user } = await renderApp(PATH);
    const mode = await screen.findByRole("combobox", { name: "How updates install" });
    expect(mode).toHaveTextContent("Manually");
    await user.click(mode);
    expect(await screen.findByRole("option", { name: /Automatically/ })).toHaveAttribute("aria-disabled", "true");
    expect(screen.queryByText("Update window")).not.toBeInTheDocument();
  });

  it("installs updates automatically in the window you set", async () => {
    signedIn();
    let current = updateSettings();
    on("GET", "instance/updates", () => updates({ settings: current }));
    history();
    on("PATCH", "instance/updates/settings", ({ body }: { body: Partial<UpdatesView["settings"]> }) => {
      current = {
        ...current,
        ...body,
        schedule: { ...current.schedule, ...(body.schedule ?? {}), timezone: "Europe/Berlin" },
        next_auto_update_at: "2026-10-18T01:00:00Z",
      };
      return updates({ settings: current });
    });
    const { user } = await renderApp(PATH);
    await choose(user, "How updates install", "Automatically");
    expect(await screen.findByText("Updates now install automatically")).toBeInTheDocument();
    expect(await screen.findByText("Update window")).toBeInTheDocument();
    expect(screen.getByText("Next 18 Oct 2026, 01:00, Europe/Berlin time.")).toBeInTheDocument();
    await choose(user, "How often", "Weekly");
    expect(await screen.findByText("Update window saved")).toBeInTheDocument();
    await choose(user, "Day", "Sunday");
    await choose(user, "Time", "05:00");
    await waitFor(() => expect(requests("PATCH", "instance/updates/settings")).toHaveLength(4));
    await choose(user, "How often", "Daily");
    await waitFor(() => expect(requests("PATCH", "instance/updates/settings")).toHaveLength(5));
    await choose(user, "How updates install", "Manually");
    expect(await screen.findByText("You now install each update yourself")).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByText("Update window")).not.toBeInTheDocument());
    expect(requests("PATCH", "instance/updates/settings").map((r) => r.body)).toEqual([
      { mode: "automatic" },
      { schedule: { frequency: "weekly", weekday: "monday", hour: 3 } },
      { schedule: { frequency: "weekly", weekday: "sunday", hour: 3 } },
      { schedule: { frequency: "weekly", weekday: "sunday", hour: 5 } },
      { schedule: { frequency: "daily", weekday: "sunday", hour: 5 } },
      { mode: "manual" },
    ]);
  });

  it("says when no window is planned and when the updater is ready without a version", async () => {
    signedIn();
    on(
      "GET",
      "instance/updates",
      updates({
        settings: updateSettings({ mode: "automatic", next_auto_update_at: null }),
        check: { last_checked_at: null, last_success_at: null, next_check_at: null, error: null },
        state: "unknown",
      }),
    );
    history();
    await renderApp(PATH);
    expect(await screen.findByText("No window planned; UTC time.")).toBeInTheDocument();
    expect(screen.getByText("Not checked yet; Kaeso looks twice a day.")).toBeInTheDocument();
  });

  it("lists past updates with who started them and how they ended", async () => {
    signedIn();
    on("GET", "instance/updates", updates());
    history([
      updateJob({ id: "upd_3", state: "rollback_failed", reason: "unhealthy", finished_at: "2026-10-15T12:05:00Z" }),
      updateJob({ id: "upd_2", state: "failed", reason: "unverified", started_by: null, finished_at: "2026-10-15T12:04:00Z" }),
      updateJob({ id: "upd_1", state: "succeeded", step: "online", from: "40.4.1", to: "40.4.2", finished_at: "2026-10-15T12:03:00Z" }),
      updateJob({
        id: "upd_0",
        state: "succeeded",
        step: "online",
        from: "40.4.0",
        to: "40.4.1",
        started_by: null,
        automatic: true,
        finished_at: "2026-10-14T03:03:00Z",
      }),
    ]);
    const { user } = await renderApp(PATH);
    const table = await screen.findByRole("table", { name: "History" });
    expect(await within(table).findByText("40.4.1 to 40.4.2")).toBeInTheDocument();
    expect(within(table).getAllByText("Updated")).toHaveLength(2);
    expect(within(table).getByText("Rollback failed")).toBeInTheDocument();
    expect(within(table).getByText("Deleted user")).toBeInTheDocument();
    expect(within(table).getByText("Automatically")).toBeInTheDocument();
    await hoverReason(user, within(table).getByText("Not installed"), "The release's signatures didn't check out, so nothing was installed.");
  });

  it("says when the update status couldn't load", async () => {
    signedIn();
    on("GET", "instance/updates", apiError(429, "rate_limited", "Too many requests; try again in a minute."));
    history();
    await renderApp(PATH);
    expect(await screen.findByText("The update status couldn't load")).toBeInTheDocument();
  });

  it("marks Updates in the settings sidebar while a release is available", async () => {
    signedIn();
    on("GET", "instance/updates", available());
    history();
    await renderApp(PATH);
    const nav = await screen.findByRole("navigation", { name: "Settings" }).catch(() => screen.findByRole("complementary", { name: "Settings" }));
    const link = await within(nav).findByRole("link", { name: /Updates/ });
    await waitFor(() => expect(within(link).getByText("New")).toBeInTheDocument());
  });
});
