import { afterEach, describe, expect, it } from "vitest";
import { apply, load } from "~/i18n";
import { updateJob } from "~/test/fixtures/instance-account";
import { STEPS, checkErrorText, jobResult, reasonText, stepLabel } from "./update-words";

afterEach(async () => apply(await load("en")));

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
  "migrated",
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

  it("says why the last look for releases failed in the person's language", async () => {
    expect(checkErrorText("api.github.com cannot be reached (timed out)")).toBe("api.github.com can't be reached: it took too long to answer");
    apply(await load("de"));
    expect(checkErrorText("api.github.com cannot be reached (no connection)")).toBe("api.github.com ist nicht erreichbar: keine Verbindung");
    expect(checkErrorText("api.github.com cannot be reached (request failed)")).toBe("api.github.com ist nicht erreichbar: die Anfrage ist fehlgeschlagen");
    expect(checkErrorText("GitHub refused more requests for now")).toBe("GitHub nimmt vorerst keine weiteren Anfragen an");
    expect(checkErrorText("api.github.com answered 502")).toBe("api.github.com hat mit 502 geantwortet");
    apply(await load("zh-CN"));
    expect(checkErrorText("the release list from api.github.com does not parse: expected value at line 1")).toBe(
      "无法读取来自 api.github.com 的版本列表：expected value at line 1",
    );
    expect(checkErrorText("the release list from api.github.com is larger than 4 MiB")).toBe("来自 api.github.com 的版本列表超过 4 MiB");
    expect(checkErrorText("no HTTP client")).toBe("网关无法创建 HTTP 客户端");
    expect(checkErrorText("something new")).toBe("something new");
  });
});
