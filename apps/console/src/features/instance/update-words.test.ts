import { describe, expect, it } from "vitest";
import { updateJob } from "~/test/fixtures/instance-account";
import { STEPS, jobResult, reasonText, stepLabel } from "./update-words";

const REASONS = [
  "manifest_unavailable",
  "manifest_invalid",
  "manual_steps",
  "major",
  "not_newer",
  "updater_outdated",
  "compose_changed",
  "compose_unwritable",
  "services_not_found",
  "pull_failed",
  "unverified",
  "docker_unavailable",
  "unhealthy",
  "interrupted",
  "updater_unreachable",
  "updater_refused",
  "state_unwritable",
];

describe("update words", () => {
  it("says every reason the gateway can send in a sentence of its own", () => {
    const texts = REASONS.map(reasonText);
    expect(new Set(texts).size).toBe(REASONS.length);
    for (const text of texts) expect(text).toMatch(/^[A-Z].*\.$/);
    expect(reasonText(null)).toBeNull();
    expect(reasonText(undefined)).toBeNull();
    expect(reasonText("cosmic_rays")).toBe("cosmic_rays");
  });

  it("names every step and every result", () => {
    expect(STEPS.map(stepLabel)).toEqual([
      "Check the release's signatures",
      "Download the images",
      "Switch to the new version",
      "Migrate the database",
      "Back online",
    ]);
    expect(jobResult(updateJob()).label).toBe("Running");
    expect(jobResult(updateJob({ state: "succeeded" }))).toEqual({ tone: "success", label: "Updated", detail: null });
    expect(jobResult(updateJob({ state: "succeeded", reason: "compose_unwritable" }))).toEqual({
      tone: "warning",
      label: "Updated",
      detail: "The updater couldn't write compose.yaml.",
    });
    expect(jobResult(updateJob({ state: "needs_attention", reason: "unhealthy" }))).toEqual({
      tone: "danger",
      label: "Needs attention",
      detail: "The new version didn't start.",
    });
    expect(jobResult(updateJob({ state: "rolled_back", reason: "unhealthy" }))).toEqual({
      tone: "warning",
      label: "Rolled back",
      detail: "The new version didn't start.",
    });
    expect(jobResult(updateJob({ state: "failed" })).label).toBe("Not installed");
    expect(jobResult(updateJob({ state: "rollback_failed" })).tone).toBe("danger");
  });
});
