import { STACK_URLS } from "./stack";
import { expect, signInAsOwner, test, toastText } from "./support";

test("a ChatGPT account becomes an OpenAI key by pasting the address the browser couldn't load", async ({ page, context }) => {
  const revokedSoFar = async () => ((await (await fetch(`${STACK_URLS.chatgpt}/__mock/revoked`)).json()) as { revoked: number }).revoked;
  const before = await revokedSoFar();
  let callback = "";
  await context.route("https://auth.openai.com/**", async (route) => {
    const asked = new URL(route.request().url());
    const answer = await fetch(`${STACK_URLS.chatgpt}${asked.pathname}${asked.search}`, { redirect: "manual" });
    callback = answer.headers.get("location") ?? "";
    await route.fulfill({ status: 200, contentType: "text/plain", body: "" });
  });
  await signInAsOwner(page);
  await expect(page).toHaveURL(/\/orgs\/[^/]+/);
  const org = /\/orgs\/([^/]+)/.exec(new URL(page.url()).pathname)?.[1] ?? "";
  await page.goto(`/orgs/${org}/providers`);
  await expect(page.getByRole("heading", { level: 1, name: "Providers" })).toBeVisible();

  await page.getByRole("button", { name: "Add provider key", exact: true }).click();
  const add = page.getByRole("dialog", { name: "Add provider key" });
  await add.getByRole("option", { name: /^OpenAI/ }).click();
  await add.getByRole("button", { name: "Continue" }).click();
  await add.getByRole("radio", { name: /^ChatGPT account/ }).click();
  await add.getByRole("button", { name: "Continue" }).click();
  const asDefault = add.getByRole("heading", { name: "Make it the OpenAI default?" });
  const signIn = add.getByRole("heading", { name: "Sign in with your ChatGPT account" });
  await expect(asDefault.or(signIn)).toBeVisible();
  if (await asDefault.isVisible()) await add.getByRole("button", { name: "Continue" }).click();
  await expect(signIn).toBeVisible();
  await expect(add.getByText("Use your own account only; sharing it breaks OpenAI's terms.")).toBeVisible();
  await add.getByRole("textbox", { name: "Name" }).fill("Owner's plan");

  const opened = page.waitForEvent("popup");
  await add.getByRole("button", { name: "Continue with ChatGPT" }).click();
  const tab = await opened;
  await expect.poll(() => callback).toMatch(/^http:\/\/127\.0\.0\.1:\d+\/auth\/callback\?code=/);
  await tab.close();
  const pasted = add.getByRole("textbox", { name: "Address of the page" });
  await expect(add.getByText("After you approve, the page can't load. That's expected. Paste its address here.")).toBeVisible();
  await pasted.fill(callback);
  await add.getByRole("button", { name: "Add account" }).click();
  await expect(toastText(page, /^(OpenAI · Owner's plan added|Owner's plan is the OpenAI default)$/)).toBeVisible();

  const row = page.getByRole("row").filter({ hasText: "Owner's plan" });
  await expect(row.getByText("owner@example.com")).toBeVisible();
  await expect(row.getByText("ChatGPT", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Actions for OpenAI · Owner's plan" }).click();
  await expect(page.getByRole("menuitem", { name: /Manage usage/ })).toHaveAttribute("href", "https://chatgpt.com/settings/usage");
  await expect(page.getByRole("menuitem", { name: "Sign in again" })).toBeVisible();
  await expect(page.getByRole("menuitem", { name: "Replace" })).toHaveCount(0);
  await page.getByRole("menuitem", { name: "Delete" }).click();
  await page.getByRole("alertdialog", { name: "Delete OpenAI · Owner's plan?" }).getByRole("button", { name: "Delete key" }).click();
  await expect(toastText(page, "OpenAI · Owner's plan deleted")).toBeVisible();
  expect(await revokedSoFar()).toBe(before + 1);
});
