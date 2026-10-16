import type { Page } from "@playwright/test";
import { expect, test, signInToProject, submitSteps } from "./support";

test.describe.configure({ mode: "serial" });

const row = (page: Page, name: string) =>
  page
    .getByRole("grid", { name: "Keys", exact: true })
    .getByRole("gridcell")
    .filter({ has: page.getByText(name, { exact: true }) });

let secret = "";

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
  const field = reveal.getByRole("textbox", { name: "Secret for checkout-web" });
  await expect(field).toHaveText(/^sk-kso-/);
  secret = (await field.textContent()) ?? "";
  await expect(reveal.getByText(`${new URL(page.url()).origin}/v1`, { exact: false }).first()).toBeVisible();
  await expect(reveal.getByText("/v1/v1")).toHaveCount(0);

  await reveal.getByRole("button", { name: "Done" }).click();
  const ask = page.getByRole("alertdialog", { name: "Did you copy the key?" });
  await expect(ask).toBeVisible();
  await ask.getByRole("button", { name: "Close without copying" }).click();
  await expect(row(page, "checkout-web")).toBeVisible();
  await expect(row(page, "checkout-web").getByText("Expires in 90 d")).toBeVisible();
});

test("the key authenticates with the gateway", async ({ request }) => {
  expect(secret).toMatch(/^sk-kso-/);
  const res = await request.get("/v1/models", { headers: { authorization: `Bearer ${secret}` } });
  expect(res.status()).toBe(200);
  const bad = await request.get("/v1/models", { headers: { authorization: "Bearer sk-kso-not-a-key" } });
  expect(bad.status()).toBe(401);
});

test("disabling a key can be undone, and a disabled key is refused", async ({ page, request }) => {
  await openKeys(page);
  await row(page, "checkout-web").click();
  const sheet = page.getByRole("dialog", { name: "checkout-web" });
  await sheet.getByRole("button", { name: "Key actions" }).click();
  await page.getByRole("menuitem", { name: "Disable" }).click();
  await expect(page.getByText("Key disabled")).toBeVisible();
  await expect(page.getByRole("grid", { name: "Keys", exact: true }).getByRole("button", { name: /^Disabled/ })).toBeVisible();
  expect((await request.get("/v1/models", { headers: { authorization: `Bearer ${secret}` } })).status()).toBe(401);

  await page.getByRole("button", { name: "Undo" }).click();
  await expect(page.getByText("Key enabled")).toBeVisible();
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
