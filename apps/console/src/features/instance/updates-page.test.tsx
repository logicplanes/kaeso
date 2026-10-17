import { describe, expect, it, vi } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { signedIn } from "~/test/fixtures";
import { available, release, updateJob, updates } from "~/test/fixtures/instance-account";
import { hoverReason } from "~/test/fixtures/org";
import { apiError, on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import type { UpdatesView } from "~/api/generated/types";

const PATH = "/settings/instance/updates";

function history(jobs: ReturnType<typeof updateJob>[] = []) {
  on("GET", "instance/updates/jobs", page(jobs));
}

describe("instance updates page", () => {
  it("says the install is up to date and when it last looked", async () => {
    signedIn();
    on("GET", "instance/updates", updates());
    history();
    await renderApp(PATH);
    expect(await screen.findByRole("heading", { name: "Updates", level: 1 })).toBeInTheDocument();
    expect(await screen.findByText("Up to date")).toBeInTheDocument();
    expect(screen.getByText("0.4.2")).toBeInTheDocument();
    expect(screen.getByText("Kaeso looks for new releases twice a day.")).toBeInTheDocument();
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
    expect(await screen.findByText("Update available")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Kaeso 0.4.3" })).toBeInTheDocument();
    const notes = screen.getByRole("region", { name: "Release notes of 0.4.3" });
    expect(within(notes).getByText("Streams keep their last chunk when a provider closes early.")).toBeInTheDocument();
    expect(within(notes).getByRole("link", { name: /On GitHub/ })).toHaveAttribute("href", "https://github.com/logicplanes/kaeso/releases/tag/v0.4.3");

    await user.click(screen.getByRole("button", { name: "Update to 0.4.3" }));
    const confirm = await screen.findByRole("dialog", { name: "Update to Kaeso 0.4.3?" });
    expect(within(confirm).getByText(/Back up PostgreSQL first/)).toBeInTheDocument();
    expect(within(confirm).getByText(/signed by Kaeso's release workflow/)).toBeInTheDocument();
    await user.click(within(confirm).getByRole("button", { name: "Update now" }));
    await waitFor(() => expect(requests("POST", "instance/updates/apply").at(-1)?.body).toEqual({ version: "0.4.3" }));

    const progress = await screen.findByRole("dialog", { name: "Updating to Kaeso 0.4.3" });
    const steps = within(progress).getByRole("group", { name: "Steps" });
    await waitFor(() => expect(within(steps).getAllByText("Done")).toHaveLength(3));
    expect(within(steps).getByText("In progress")).toBeInTheDocument();
    expect(within(steps).getByText("Waiting")).toBeInTheDocument();
    expect(await screen.findByRole("dialog", { name: "Kaeso 0.4.3 is running" }, { timeout: 5_000 })).toBeInTheDocument();
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
    await user.click(await screen.findByRole("button", { name: "Update to 0.4.3" }));
    await user.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(requests("POST", "instance/updates/apply")).toHaveLength(0);
    await user.click(screen.getByRole("button", { name: "Update to 0.4.3" }));
    await screen.findByRole("dialog", { name: "Update to Kaeso 0.4.3?" });
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Update to 0.4.3" }));
    await screen.findByRole("dialog", { name: "Update to Kaeso 0.4.3?" });
    await user.keyboard("{Control>}{Enter}{/Control}");
    await waitFor(() => expect(requests("POST", "instance/updates/apply")).toHaveLength(1));
    expect(await screen.findByRole("dialog", { name: "Updating to Kaeso 0.4.3" })).toBeInTheDocument();
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
    expect(await screen.findByText("Updating to Kaeso 0.4.3")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Show progress" }));
    const dialog = await screen.findByRole("dialog");
    expect(await within(dialog).findByText("The new version didn't start.")).toBeInTheDocument();
    expect(within(dialog).getByText("Kaeso 0.4.2 runs again, unchanged.")).toBeInTheDocument();
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
        reason: "unhealthy",
        detail: "the new gateway was not healthy within 600 seconds",
        finished_at: "2026-10-15T12:02:00Z",
      }),
    );
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Show progress" }));
    const dialog = await screen.findByRole("dialog");
    expect(await within(dialog).findByText("Needs attention")).toBeInTheDocument();
    expect(
      within(dialog).getByText("The new gateway may have migrated the database, so Kaeso 0.4.2 wasn't started again; follow the upgrade guide."),
    ).toBeInTheDocument();
    expect(within(dialog).queryByText("Kaeso 0.4.2 runs again, unchanged.")).not.toBeInTheDocument();
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
        detail: "0.4.3 runs, but compose.yaml cannot be written: read-only file system",
        finished_at: "2026-10-15T12:02:00Z",
      }),
    );
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Show progress" }));
    const dialog = await screen.findByRole("dialog");
    expect(await within(dialog).findByText("Kaeso 0.4.3 is running")).toBeInTheDocument();
    expect(within(dialog).getByText("One step is left")).toBeInTheDocument();
    expect(within(dialog).getByText("Set the Kaeso images in compose.yaml to 0.4.3 by hand.")).toBeInTheDocument();
  });

  it("sends a major release to the upgrade guide", async () => {
    signedIn();
    on(
      "GET",
      "instance/updates",
      available({
        manual: "major",
        can_apply: false,
        latest: release({ version: "1.0.0", major: true }),
        releases: [release({ version: "1.0.0", major: true })],
      }),
    );
    history();
    await renderApp(PATH);
    expect(await screen.findByText("Update by hand")).toBeInTheDocument();
    expect(screen.getByText("Kaeso 1.0.0 is a new major version: update by hand with the upgrade guide.")).toBeInTheDocument();
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
    [{ updater: { state: "outdated", version: "0.4.0" } }, "The updater is too old for this release; update by hand once."],
    [{ settings: { check: true, channel: "stable", apply: false } }, "Turn on one-click updates below to update from here."],
  ] as const)("shows the commands to update by hand when one click can't (%#)", async (change, reason) => {
    signedIn();
    on("GET", "instance/updates", available({ can_apply: false, ...change }));
    history();
    await renderApp(PATH);
    expect(await screen.findByText(reason)).toBeInTheDocument();
    expect(screen.getByText(/releases\/download\/v0\.4\.3\/compose\.yaml/)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /Set up one-click updates/ })).toHaveAttribute(
      "href",
      "https://docs.kaeso.app/self-hosting/upgrades#one-click-updates",
    );
  });

  it("says when the last check failed and checks again on request", async () => {
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
    on("POST", "instance/updates/check", available());
    const { user } = await renderApp(PATH);
    const failed = await screen.findByText(/^Failed /);
    expect(screen.getByText("GitHub couldn't be reached; Kaeso tries again later.")).toBeInTheDocument();
    expect(screen.getByText("Not checked yet")).toBeInTheDocument();
    await hoverReason(user, failed, "api.github.com cannot be reached (no connection)");
    await user.click(screen.getByRole("button", { name: "Check now" }));
    expect(await screen.findByText("Kaeso 0.4.3 is available")).toBeInTheDocument();
    expect(requests("POST", "instance/updates/check")).toHaveLength(1);
  });

  it.each([
    [null, "Kaeso is up to date"],
    ["api.github.com answered 500", "Releases couldn't be fetched; try again later"],
  ])("toasts what a check found (%s)", async (error, toast) => {
    signedIn();
    on("GET", "instance/updates", updates({ check: { last_checked_at: null, last_success_at: null, next_check_at: null, error: null } }));
    history();
    on("POST", "instance/updates/check", updates({ check: { last_checked_at: null, last_success_at: null, next_check_at: null, error } }));
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("Never")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Check now" }));
    expect(await screen.findByText(toast)).toBeInTheDocument();
  });

  it("changes the settings and says what changed", async () => {
    signedIn();
    let current: UpdatesView["settings"] = { check: false, channel: "stable", apply: false };
    on("GET", "instance/updates", () => updates({ settings: current }));
    history();
    on("PATCH", "instance/updates/settings", ({ body }: { body: Partial<UpdatesView["settings"]> }) => {
      current = { ...current, ...body };
      return updates({ settings: current });
    });
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("Automatic checks are off.")).toBeInTheDocument();
    await user.click(screen.getByRole("switch", { name: "Look for new releases" }));
    expect(await screen.findByText("Kaeso looks for releases again")).toBeInTheDocument();
    await user.click(screen.getByRole("switch", { name: "One-click updates" }));
    expect(await screen.findByText("One-click updates are on")).toBeInTheDocument();
    await user.click(screen.getByRole("combobox", { name: "Releases" }));
    await user.click(await screen.findByRole("option", { name: "With pre-releases" }));
    expect(await screen.findByText("Channel saved")).toBeInTheDocument();
    await user.click(screen.getByRole("switch", { name: "Look for new releases" }));
    expect(await screen.findByText("Kaeso stopped looking for releases")).toBeInTheDocument();
    await user.click(screen.getByRole("switch", { name: "One-click updates" }));
    expect(await screen.findByText("One-click updates are off")).toBeInTheDocument();
    await user.click(screen.getByRole("combobox", { name: "Releases" }));
    await user.click(await screen.findByRole("option", { name: "Stable only" }));
    await waitFor(() => expect(requests("PATCH", "instance/updates/settings")).toHaveLength(6));
    expect(requests("PATCH", "instance/updates/settings").map((r) => r.body)).toEqual([
      { check: true },
      { apply: true },
      { channel: "prerelease" },
      { check: false },
      { apply: false },
      { channel: "stable" },
    ]);
  });

  it("lists past updates with who started them and how they ended", async () => {
    signedIn();
    on("GET", "instance/updates", updates());
    history([
      updateJob({ id: "upd_3", state: "rollback_failed", reason: "unhealthy", finished_at: "2026-10-15T12:05:00Z" }),
      updateJob({ id: "upd_2", state: "failed", reason: "unverified", started_by: null, finished_at: "2026-10-15T12:04:00Z" }),
      updateJob({ id: "upd_1", state: "succeeded", step: "online", from: "0.4.1", to: "0.4.2", finished_at: "2026-10-15T12:03:00Z" }),
    ]);
    const { user } = await renderApp(PATH);
    const table = await screen.findByRole("table", { name: "History" });
    expect(await within(table).findByText("0.4.1 to 0.4.2")).toBeInTheDocument();
    expect(within(table).getByText("Updated")).toBeInTheDocument();
    expect(within(table).getByText("Rollback failed")).toBeInTheDocument();
    expect(within(table).getByText("Deleted user")).toBeInTheDocument();
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
