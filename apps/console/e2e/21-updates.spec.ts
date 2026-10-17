import type { Page } from "@playwright/test";
import { STACK_URLS } from "./stack";
import { expect, signInAsOwner, test, toastText } from "./support";

test.describe.configure({ mode: "serial" });

type Scenario = {
  releases?: "patch" | "major" | "manual" | "none" | "error";
  updater?: "ready" | "down";
  outcome?: "succeeded" | "rolled_back" | "needs_attention";
};

async function scenario(s: Scenario): Promise<{ running: string; versions: { patch: string; minor: string; major: string } }> {
  const res = await fetch(`${STACK_URLS.releases}/__scenario`, { method: "POST", body: JSON.stringify(s) });
  return (await res.json()) as { running: string; versions: { patch: string; minor: string; major: string } };
}

async function openUpdates(page: Page): Promise<void> {
  await signInAsOwner(page);
  await page.goto("/settings/instance/updates");
  await expect(page.getByRole("heading", { level: 1, name: "Updates" })).toBeVisible();
}

async function checkNow(page: Page): Promise<void> {
  await page.getByRole("button", { name: "Check now" }).click();
}

test("up to date: a check finds nothing newer", async ({ page }) => {
  await scenario({ releases: "none" });
  await openUpdates(page);
  await checkNow(page);
  await expect(toastText(page, "Kaeso is up to date")).toBeVisible();
  await expect(page.getByText("Up to date", { exact: true })).toBeVisible();
  await expect(page.getByText("No updates yet")).toBeVisible();
});

test("check failed: the reason shows and nothing changes", async ({ page }) => {
  await scenario({ releases: "error" });
  await openUpdates(page);
  await checkNow(page);
  await expect(toastText(page, "Releases couldn't be fetched; try again later")).toBeVisible();
  await expect(page.getByText("GitHub couldn't be reached; Kaeso tries again later.")).toBeVisible();
});

test("one click: an update is confirmed, followed and recorded", async ({ page }) => {
  const { versions } = await scenario({ releases: "patch" });
  await openUpdates(page);
  await checkNow(page);
  await expect(toastText(page, `Kaeso ${versions.patch} is available`)).toBeVisible();
  await expect(page.getByText("Turn on one-click updates below to update from here.")).toBeVisible();
  await expect(page.locator(`nav[aria-label="Settings"]`).getByText("New", { exact: true })).toBeVisible();
  await page.getByRole("switch", { name: "One-click updates" }).click();
  await expect(toastText(page, "One-click updates are on")).toBeVisible();
  await page.getByRole("button", { name: `Update to ${versions.patch}` }).click();
  const confirm = page.getByRole("dialog", { name: `Update to Kaeso ${versions.patch}?` });
  await expect(confirm.getByText(/Back up PostgreSQL first/)).toBeVisible();
  await confirm.getByRole("button", { name: "Update now" }).click();
  await expect(page.getByRole("dialog", { name: `Kaeso ${versions.patch} is running` })).toBeVisible({ timeout: 30_000 });
  await page.getByRole("dialog").getByRole("button", { name: "Close" }).first().click();
  const history = page.getByRole("table", { name: "History" });
  await expect(history.getByText("Updated")).toBeVisible();
  await expect(history.getByText("Ada Okafor")).toBeVisible();
});

test("rolled back: the failure says what runs now", async ({ page }) => {
  const { running } = await scenario({ releases: "patch", outcome: "rolled_back" });
  await openUpdates(page);
  await checkNow(page);
  await page.getByRole("button", { name: /^Update to / }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Update now" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByText("The new version didn't start.")).toBeVisible({ timeout: 30_000 });
  await expect(dialog.getByText(`Kaeso ${running} runs again, unchanged.`)).toBeVisible();
});

test("needs attention: a gateway that may have migrated is left for a person", async ({ page }) => {
  const { running } = await scenario({ releases: "patch", outcome: "needs_attention" });
  await openUpdates(page);
  await checkNow(page);
  await page.getByRole("button", { name: /^Update to / }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Update now" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByText("Needs attention")).toBeVisible({ timeout: 30_000 });
  await expect(
    dialog.getByText(`The new gateway may have migrated the database, so Kaeso ${running} wasn't started again; follow the upgrade guide.`),
  ).toBeVisible();
  await expect(dialog.getByText(`Kaeso ${running} runs again, unchanged.`)).toHaveCount(0);
});

test("major release: the upgrade guide instead of a button", async ({ page }) => {
  const { versions } = await scenario({ releases: "major" });
  await openUpdates(page);
  await checkNow(page);
  await expect(page.getByText(`Kaeso ${versions.major} is a new major version: update by hand with the upgrade guide.`)).toBeVisible();
  await expect(page.getByRole("link", { name: /Open the upgrade guide/ })).toBeVisible();
  await expect(page.getByRole("button", { name: /^Update to / })).toHaveCount(0);
});

test("manual steps: a release that asks for them is never one click", async ({ page }) => {
  await scenario({ releases: "manual" });
  await openUpdates(page);
  await checkNow(page);
  await expect(page.getByText("A release on the way asks for steps by hand: follow the upgrade guide.")).toBeVisible();
});

test("updater away: the commands to update by hand", async ({ page }) => {
  const { versions } = await scenario({ releases: "patch", updater: "down" });
  await openUpdates(page);
  await checkNow(page);
  await expect(page.getByText("The updater doesn't answer; check that its container runs.")).toBeVisible();
  await expect(page.getByText(`releases/download/v${versions.patch}/compose.yaml`, { exact: false })).toBeVisible();
});

test("settings: checking turns on and off, and pre-releases are offered", async ({ page }) => {
  await scenario({ releases: "patch" });
  await openUpdates(page);
  const check = page.getByRole("switch", { name: "Look for new releases" });
  await expect(check).not.toBeChecked();
  await check.click();
  await expect(toastText(page, "Kaeso looks for releases again")).toBeVisible();
  await expect(check).toBeChecked();
  await check.click();
  await expect(toastText(page, "Kaeso stopped looking for releases")).toBeVisible();
  await expect(check).not.toBeChecked();
  await expect(page.getByText("Automatic checks are off.")).toBeVisible();
  await page.getByRole("combobox", { name: "Releases" }).click();
  await page.getByRole("option", { name: "With pre-releases" }).click();
  await expect(toastText(page, "Channel saved")).toBeVisible();
  await checkNow(page);
  await expect(page.getByText("Pre-release", { exact: true })).toBeVisible();
});
