import { STACK_URLS } from "./stack";
import { expect, signInAsOwner, test, toastText } from "./support";

test("a Grok account becomes a SpaceX key once its code is approved on another device", async ({ page, context }) => {
  const revokedSoFar = async () => ((await (await fetch(`${STACK_URLS.accounts}/__mock/grok/revoked`)).json()) as { revoked: number }).revoked;
  const before = await revokedSoFar();
  await context.route("https://accounts.x.ai/**", async (route) => {
    const asked = new URL(route.request().url());
    const answer = await fetch(`${STACK_URLS.accounts}${asked.pathname}${asked.search}`);
    await route.fulfill({ status: answer.status, contentType: "text/plain", body: await answer.text() });
  });
  await signInAsOwner(page);
  await expect(page).toHaveURL(/\/orgs\/[^/]+/);
  const org = /\/orgs\/([^/]+)/.exec(new URL(page.url()).pathname)?.[1] ?? "";
  await page.goto(`/orgs/${org}/providers`);
  await expect(page.getByRole("heading", { level: 1, name: "Providers" })).toBeVisible();

  await page.getByRole("button", { name: "Add provider key", exact: true }).click();
  const add = page.getByRole("dialog", { name: "Add provider key" });
  await add.getByRole("option", { name: /^SpaceX/ }).click();
  await add.getByRole("button", { name: "Continue" }).click();
  const grok = add.getByRole("radio", { name: /^Grok account/ });
  await expect(add.getByText("Experimental", { exact: true })).toBeVisible();
  await grok.click();
  await add.getByRole("button", { name: "Continue" }).click();
  const asDefault = add.getByRole("heading", { name: "Make it the SpaceX default?" });
  const signIn = add.getByRole("heading", { name: "Sign in with your Grok account" });
  await expect(asDefault.or(signIn)).toBeVisible();
  if (await asDefault.isVisible()) await add.getByRole("button", { name: "Continue" }).click();
  await expect(signIn).toBeVisible();
  await add.getByRole("textbox", { name: "Name" }).fill("Owner's Grok");
  await add.getByRole("button", { name: "Get a sign-in code" }).click();

  const status = add.getByRole("status");
  await expect(status).toContainText("Waiting for your approval; the code expires in");
  const qr = add.getByRole("img", { name: "QR code to open the Grok sign-in page" });
  await expect(qr).toBeVisible();
  expect(await qr.boundingBox()).toMatchObject({ width: 160, height: 160 });
  await expect(add.getByRole("button", { name: "Copy code" })).toBeVisible();
  const link = add.getByRole("link", { name: /^accounts\.x\.ai/ });
  const code = (await link.getAttribute("href"))?.split("user_code=")[1] ?? "";
  expect(code).toMatch(/^[0-9A-F]{4}-[0-9A-F]{4}$/);
  await expect(add.getByText(code, { exact: true })).toBeVisible();

  const opened = page.waitForEvent("popup");
  await link.click();
  const tab = await opened;
  await expect(tab.locator("body")).toHaveText("Approved.");
  await tab.close();
  await expect(toastText(page, /^(SpaceX · Owner's Grok added|Owner's Grok is the SpaceX default)$/)).toBeVisible();

  const row = page.getByRole("row").filter({ hasText: "Owner's Grok" });
  await expect(row.getByText("owner@example.com")).toBeVisible();
  await expect(row.getByText("Grok", { exact: true })).toBeVisible();
  await expect(row.getByText(/Experimental/)).toBeVisible();
  await page.getByRole("button", { name: "Actions for SpaceX · Owner's Grok" }).click();
  await expect(page.getByRole("menuitem", { name: "Sign in again" })).toBeVisible();
  await expect(page.getByRole("menuitem", { name: /Manage usage/ })).toHaveCount(0);
  await expect(page.getByRole("menuitem", { name: "Replace" })).toHaveCount(0);
  await page.getByRole("menuitem", { name: "Delete" }).click();
  await page.getByRole("alertdialog", { name: "Delete SpaceX · Owner's Grok?" }).getByRole("button", { name: "Delete key" }).click();
  await expect(toastText(page, "SpaceX · Owner's Grok deleted")).toBeVisible();
  expect(await revokedSoFar()).toBe(before + 1);
});
