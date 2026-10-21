import type { Page } from "@playwright/test";
import { expect, test, signInAsOwner, toastText } from "./support";

test.describe.configure({ mode: "serial" });

async function openInstance(page: Page, label: string) {
  await signInAsOwner(page);
  await page.goto("/settings/instance");
  await expect(page).toHaveURL(/\/settings\/instance\/general$/);
  if (label !== "General") await page.locator(`nav[aria-label="Settings"] a[href^="/settings/instance/"]`).getByText(label, { exact: true }).click();
  await expect(page.getByRole("heading", { level: 1, name: label })).toBeVisible();
}

test("users: one is invited, deactivated and reactivated", async ({ page }) => {
  await openInstance(page, "Users");
  await page.getByRole("button", { name: "Invite user" }).click();
  const invite = page.getByRole("dialog", { name: "Invite user" });
  await invite.getByLabel("Email").fill("someone@example.com");
  await invite.getByRole("button", { name: "Send invitation" }).click();
  await expect(toastText(page, "Invitation sent to someone@example.com")).toBeVisible();

  await page.getByRole("table", { name: "Users" }).getByText("Lin Park").click();
  const sheet = page.getByRole("dialog", { name: "Lin Park" });
  await sheet.getByRole("button", { name: "Deactivate" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Deactivate" }).click();
  await expect(toastText(page, "Lin Park deactivated")).toBeVisible();
  await expect(page.getByRole("table", { name: "Users" }).getByText("Deactivated")).toBeVisible();
  await sheet.getByRole("button", { name: "Reactivate" }).click();
  await expect(toastText(page, "Lin Park reactivated")).toBeVisible();
});

test("organizations: one is created with an owner and opens with its metadata", async ({ page }) => {
  await openInstance(page, "Organizations");
  await page.getByRole("button", { name: "Create organization" }).click();
  const dialog = page.getByRole("dialog", { name: "Create organization" });
  await dialog.getByLabel("Name", { exact: true }).fill("Contoso");
  await dialog.getByRole("combobox", { name: "Owner (optional)" }).fill("Lin");
  await page.getByRole("option", { name: /Lin Park/ }).click();
  await dialog.getByRole("button", { name: "Create organization" }).click();
  await expect(toastText(page, "Contoso created")).toBeVisible();
  await expect(page.getByRole("heading", { level: 1, name: "Contoso" })).toBeVisible();
  await expect(page.getByText("You see its metadata, not its content; changes go in its audit log.")).toBeVisible();
  await expect(page.getByLabel("Usage in the last 30 days")).toBeVisible();
  await expect(page.getByRole("grid", { name: "Members" }).getByText("Lin Park")).toBeVisible();
  await expect(page.getByRole("link", { name: "Open organization" })).toHaveCount(0);
  await page.getByRole("button", { name: "Add owner" }).click();
  await page.getByRole("dialog", { name: "Add an owner" }).getByRole("button", { name: "Add me" }).click();
  await expect(toastText(page, "You're an owner now")).toBeVisible();
  await expect(page.getByRole("link", { name: "Open organization" })).toBeVisible();
});

test("logging and system: retention defaults change and a test email is sent", async ({ page }) => {
  await openInstance(page, "Logging");
  await page.getByRole("textbox", { name: "Default content retention" }).fill("14");
  await page.getByRole("button", { name: "Save default content retention" }).click();
  await expect(toastText(page, "Retention saved")).toBeVisible();
  await page.locator(`nav[aria-label="Settings"] a[href="/settings/instance/system"]`).click();
  await page.getByRole("button", { name: "Send test email" }).click();
  await page.getByRole("dialog", { name: "Send a test email" }).getByRole("button", { name: "Send test email" }).click();
  await expect(toastText(page, "Test email sent to owner@example.com")).toBeVisible();
});

test("general: the deletion grace period changes, and with deletion off no organization offers it", async ({ page }) => {
  await openInstance(page, "General");
  await page.getByRole("textbox", { name: "Grace period" }).fill("3");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(toastText(page, "Deletions wait 3 days")).toBeVisible();

  await page.getByRole("switch", { name: "Organizations can be deleted" }).click();
  await expect(toastText(page, "Organizations can't be deleted")).toBeVisible();
  await page.goto("/");
  await page.getByRole("button", { name: /: switch or open settings$/ }).click();
  await page.getByRole("menuitem", { name: "Organization settings" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "General" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Transfer ownership" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Delete organization" })).toHaveCount(0);

  await page.goto("/settings/instance/general");
  await page.getByRole("switch", { name: "Organizations can be deleted" }).click();
  await expect(toastText(page, "Organizations can be deleted")).toBeVisible();
  await page.getByRole("textbox", { name: "Grace period" }).fill("7");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(toastText(page, "Deletions wait 7 days")).toBeVisible();
});

test("model sources, system and the instance audit log", async ({ page }) => {
  await openInstance(page, "Model sources");
  const source = page.getByRole("table", { name: "Model sources" }).getByRole("row", { name: /models\.dev/ });
  await expect(source.getByText("In use")).toBeVisible();
  await expect(source.getByText(/\d+ models/)).toBeVisible();
  await page.getByRole("button", { name: "Sync now" }).click();
  await expect(toastText(page, /Synced: \d+ models|models\.dev couldn't be fetched/)).toBeVisible({ timeout: 30_000 });

  await page.locator(`nav[aria-label="Settings"] a[href="/settings/instance/system"]`).click();
  await expect(page.getByText("Ready")).toBeVisible();
  await expect(page.getByText("PostgreSQL").first()).toBeVisible();
  await expect(page.getByRole("button", { name: "Check now" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Send test email" })).toBeVisible();

  await page.locator(`nav[aria-label="Settings"] a[href="/settings/instance/audit-log"]`).click();
  await expect(page.getByRole("heading", { level: 1, name: "Audit log" })).toBeVisible();
  await expect(page.getByRole("table", { name: "Audit log" }).getByText("instance.settings_update").first()).toBeVisible();
});
