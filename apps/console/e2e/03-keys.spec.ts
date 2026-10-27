import type { Page } from "@playwright/test";
import { expect, test, signInToProject, stepTo, submitSteps } from "./support";

test.describe.configure({ mode: "serial" });

const row = (page: Page, name: string) =>
  page
    .getByRole("grid", { name: "Keys", exact: true })
    .getByRole("gridcell")
    .filter({ has: page.getByText(name, { exact: true }) });

let secret = "";
let preview = "";

async function openKeys(page: Page) {
  await signInToProject(page);
  await page.getByRole("link", { name: "API keys" }).click();
  await expect(page).toHaveTitle(/^API keys · /);
}

test("a new key is shown once, and closing without copying asks first", async ({ page }) => {
  await openKeys(page);
  await expect(page.getByText("No keys yet")).toBeVisible();
  await page.getByRole("button", { name: "Create key" }).first().click();
  const dialog = page.getByRole("dialog", { name: "Create key" });
  await dialog.getByLabel("Name").fill("checkout-web");
  await submitSteps(dialog, "Create key");

  const reveal = page.getByRole("dialog", { name: "Key created" });
  const field = reveal.getByRole("textbox", { name: "Key for checkout-web" });
  await expect(field).toHaveText(/^sk-kso-/);
  secret = (await field.textContent()) ?? "";
  await expect(reveal.getByText(`${new URL(page.url()).origin}/v1`, { exact: false }).first()).toBeVisible();
  await expect(reveal.getByText("/v1/v1")).toHaveCount(0);

  await reveal.getByRole("button", { name: "Finish" }).click();
  const ask = page.getByRole("alertdialog", { name: "Did you copy the key?" });
  await expect(ask).toBeVisible();
  await ask.getByRole("button", { name: "Close without copying" }).click();
  await expect(row(page, "checkout-web")).toBeVisible();
  await expect(row(page, "checkout-web").getByText("Expires in 90 days")).toBeVisible();
});

test("the next key starts on an empty first step, and the last key's secret is no longer on the page", async ({ page }) => {
  expect(secret).toMatch(/^sk-kso-/);
  await openKeys(page);
  const create = page.getByRole("button", { name: "Create key" }).first();
  const dialog = page.getByRole("dialog", { name: "Create key" });
  await create.click();
  await expect(dialog.getByText("Step 1 of 4")).toBeVisible();
  await expect(dialog.getByLabel("Name")).toHaveValue("");
  expect(await page.content()).not.toContain(secret);

  await dialog.getByLabel("Name").fill("deploy-preview");
  await submitSteps(dialog, "Create key");
  const reveal = page.getByRole("dialog", { name: "Key created" });
  const field = reveal.getByRole("textbox", { name: "Key for deploy-preview" });
  await expect(field).toHaveText(/^sk-kso-/);
  const next = (await field.textContent()) ?? "";
  preview = next;
  expect(next).not.toBe(secret);
  await reveal.getByRole("button", { name: "Finish" }).click();
  await page.getByRole("alertdialog", { name: "Did you copy the key?" }).getByRole("button", { name: "Close without copying" }).click();
  await expect(reveal).toBeHidden();
  await expect(row(page, "deploy-preview")).toBeVisible();
  expect(await page.content()).not.toContain(next);

  await create.click();
  await expect(dialog.getByText("Step 1 of 4")).toBeVisible();
  await expect(dialog.getByRole("heading", { name: "What is it for?" })).toBeVisible();
  await expect(dialog.getByLabel("Name")).toHaveValue("");
  await expect(reveal).toHaveCount(0);
  expect(await page.content()).not.toContain(next);
  await dialog.getByLabel("Name").fill("half-way");
  await stepTo(dialog, dialog.getByRole("heading", { name: "What can it call?" }));
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();

  await create.click();
  await expect(dialog.getByText("Step 1 of 4")).toBeVisible();
  await expect(dialog.getByLabel("Name")).toHaveValue("");
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
});

test("the key authenticates with the gateway", async ({ request }) => {
  expect(secret).toMatch(/^sk-kso-/);
  const res = await request.get("/v1/models", { headers: { authorization: `Bearer ${secret}` } });
  expect(res.status()).toBe(200);
  const bad = await request.get("/v1/models", { headers: { authorization: "Bearer sk-kso-not-a-key" } });
  expect(bad.status()).toBe(401);
});

test("rotating a key shows the new key once, and the old key works until the grace period ends", async ({ page, request }) => {
  expect(preview).toMatch(/^sk-kso-/);
  await openKeys(page);
  await row(page, "deploy-preview").click();
  const sheet = page.getByRole("dialog", { name: "deploy-preview" });
  await sheet.getByRole("button", { name: "Key actions" }).click();
  await page.getByRole("menuitem", { name: "Rotate" }).click();
  const dialog = page.getByRole("dialog", { name: "Rotate deploy-preview" });
  await expect(dialog.getByRole("heading", { name: "When does the old key stop working?" })).toBeVisible();
  await dialog.getByRole("radio", { name: "After 1 hour" }).check();
  await submitSteps(dialog, "Rotate key");

  const reveal = page.getByRole("dialog", { name: "Key rotated" });
  await expect(reveal.getByText("Copy the new key now: it's shown once, and the old key stops in 1 hour.")).toBeVisible();
  const field = reveal.getByRole("textbox", { name: "Key for deploy-preview" });
  await expect(field).toHaveText(/^sk-kso-/);
  const rotated = (await field.textContent()) ?? "";
  expect(rotated).not.toBe(preview);
  await reveal.getByRole("button", { name: "Copy key" }).click();
  await reveal.getByRole("button", { name: "Finish" }).click();
  await expect(reveal).toBeHidden();

  for (const key of [preview, rotated]) expect((await request.get("/v1/models", { headers: { authorization: `Bearer ${key}` } })).status()).toBe(200);
  await expect(sheet.getByText("Replaced by")).toBeVisible();
  await expect(row(page, "deploy-preview").getByText(/^Replaced; stops in/)).toBeVisible();
  await sheet.getByRole("link", { name: /^sk-kso-/ }).click();
  await expect(sheet.getByText("Rotated from")).toBeVisible();
});

test("disabling a key can be undone, and a disabled key is refused", async ({ page, request }) => {
  await openKeys(page);
  await row(page, "checkout-web").click();
  const sheet = page.getByRole("dialog", { name: "checkout-web" });
  await sheet.getByRole("button", { name: "Key actions" }).click();
  await page.getByRole("menuitem", { name: "Disable" }).click();
  await expect(page.getByText("Key disabled")).toBeVisible();
  await expect(row(page, "checkout-web")).toHaveCount(0);
  expect((await request.get("/v1/models", { headers: { authorization: `Bearer ${secret}` } })).status()).toBe(401);

  await page.getByRole("button", { name: "Undo" }).click();
  await expect(page.getByText("Key enabled")).toBeVisible();
  await expect(row(page, "checkout-web")).toBeVisible();
  expect((await request.get("/v1/models", { headers: { authorization: `Bearer ${secret}` } })).status()).toBe(200);
});

test("archiving says what happens and moves the key out of the list", async ({ page }) => {
  await openKeys(page);
  await row(page, "checkout-web").click();
  const sheet = page.getByRole("dialog", { name: "checkout-web" });
  await sheet.getByRole("button", { name: "Key actions" }).click();
  await page.getByRole("menuitem", { name: "Archive" }).click();
  const confirm = page.getByRole("alertdialog", { name: "Archive checkout-web?" });
  await expect(confirm.getByText(/^Applications using it stop authenticating/)).toBeVisible();
  await confirm.getByRole("button", { name: "Archive key" }).click();
  await expect(page.getByText("Key archived")).toBeVisible();
  await expect(row(page, "checkout-web")).toHaveCount(0);
  await page.getByRole("tablist", { name: "Keys" }).getByRole("tab", { name: "All", exact: true }).click();
  await expect(row(page, "checkout-web")).toBeVisible();
});
