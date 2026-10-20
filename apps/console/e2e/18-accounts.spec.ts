import type { Browser, Page } from "@playwright/test";
import { expect, test, NEW_PASSWORD, OWNER, clearMail, enterEmail, expectSignedIn, mailLink, signInWithPassword, toastText } from "./support";

test.describe.configure({ mode: "serial" });

async function openAccounts(page: Page) {
  await signInWithPassword(page, OWNER.email, NEW_PASSWORD);
  await expectSignedIn(page);
  await page.goto("/settings/instance/general");
  await expect(page.getByRole("heading", { level: 2, name: "Accounts" })).toBeVisible();
}

async function choose(page: Page, select: "Account creation" | "Organization creation", option: string) {
  await page.getByRole("combobox", { name: select }).click();
  await page.getByRole("option", { name: new RegExp(`^${option}`) }).click();
}

async function methodsFor(browser: Browser, email: string) {
  const context = await browser.newContext();
  const page = await context.newPage();
  await page.goto("/sign-in");
  await enterEmail(page, email);
  await expect(page.getByText(email, { exact: true })).toBeVisible();
  return { context, page };
}

test("invitation only: no address is offered an account", async ({ page, browser }) => {
  await openAccounts(page);
  await expect(page.getByRole("combobox", { name: "Account creation" })).toHaveText(/Invitation only/);
  await expect(page.getByText("Allowed domains", { exact: true })).toHaveCount(0);
  const { context, page: visitor } = await methodsFor(browser, "ada@example.org");
  await expect(visitor.getByRole("link", { name: "Create an account" })).toHaveCount(0);
  await visitor.goto("/sign-up");
  await expect(visitor.getByRole("heading", { name: "Accounts here are by invitation" })).toBeVisible();
  await context.close();
});

test("allowed email domains: those addresses sign up, others are refused with the gateway's reason", async ({ page, browser }) => {
  await openAccounts(page);
  await choose(page, "Account creation", "Allowed email domains");
  const dialog = page.getByRole("dialog", { name: "Allow email domains" });
  const domains = dialog.getByLabel("Allowed domains");
  await domains.pressSequentially("Example.ORG,*.example.net,");
  await expect(dialog.getByText("*.example.net", { exact: true })).toBeVisible();
  await dialog.getByRole("button", { name: "Allow domains" }).click();
  await expect(toastText(page, "Allowed email domains can sign up")).toBeVisible();
  await expect(page.getByRole("combobox", { name: "Account creation" })).toHaveText(/Allowed email domains/);
  await expect(page.getByText("*.example.net, example.org", { exact: true })).toBeVisible();

  await clearMail();
  const { context, page: visitor } = await methodsFor(browser, "ada@eu.example.net");
  await visitor.getByRole("link", { name: "Create an account" }).click();
  await expect(visitor.getByRole("heading", { name: "Check your email" })).toBeVisible();
  await expect(visitor.getByText("Open the link we sent to ada@eu.example.net to continue.")).toBeVisible();
  const link = await mailLink("ada@eu.example.net", /\/auth\/signup#token=/);
  await visitor.goto(new URL(link).pathname + new URL(link).hash);
  await visitor.getByLabel("Name").fill("Ada Example");
  await visitor.getByLabel("Password", { exact: true }).fill(NEW_PASSWORD);
  await visitor.getByRole("button", { name: "Create account" }).click();
  await expect(visitor).not.toHaveURL(/sign-up|signup/);
  await context.close();

  const other = await methodsFor(browser, "bob@example.net");
  await expect(other.page.getByRole("link", { name: "Create an account" })).toHaveCount(0);
  await other.page.goto("/sign-up");
  await other.page.getByLabel("Email").fill("bob@example.net");
  await other.page.getByRole("button", { name: "Continue", exact: true }).click();
  await expect(toastText(other.page, "This email address can't create an account here; ask an administrator to invite you.")).toBeVisible();
  await other.context.close();
});

test("organization creation follows the allowed domains, which are edited in place", async ({ page, browser }) => {
  await openAccounts(page);
  await choose(page, "Organization creation", "Users on allowed domains");
  await expect(toastText(page, "Allowed email domains can create organizations")).toBeVisible();

  const ada = await browser.newContext();
  const adaPage = await ada.newPage();
  await signInWithPassword(adaPage, "ada@eu.example.net", NEW_PASSWORD);
  await expect(adaPage).not.toHaveURL(/sign-in/);
  const session = (await (await adaPage.request.get("/platform/v1/auth/session")).json()) as { can_create_orgs: boolean };
  expect(session.can_create_orgs).toBe(true);

  await page.getByRole("button", { name: "Edit domains" }).click();
  const dialog = page.getByRole("dialog", { name: "Allowed domains" });
  await dialog.getByRole("button", { name: /Remove \*\.example\.net/ }).click();
  await dialog.getByRole("button", { name: "Save domains" }).click();
  await expect(toastText(page, "Allowed domains saved")).toBeVisible();
  await expect(page.getByText("example.org", { exact: true })).toBeVisible();
  const after = (await (await adaPage.request.get("/platform/v1/auth/session")).json()) as { can_create_orgs: boolean };
  expect(after.can_create_orgs).toBe(false);
  await ada.close();
});

test("anyone: every address is offered an account; invitation only closes it again", async ({ page, browser }) => {
  await openAccounts(page);
  await choose(page, "Account creation", "Anyone");
  await expect(toastText(page, "Anyone can create an account")).toBeVisible();
  const visitor = await methodsFor(browser, "grace@example.org");
  await expect(visitor.page.getByRole("link", { name: "Create an account" })).toBeVisible();
  await visitor.context.close();

  await choose(page, "Organization creation", "Instance admins only");
  await expect(toastText(page, "Only instance admins can create organizations")).toBeVisible();
  await choose(page, "Account creation", "Invitation only");
  await expect(toastText(page, "Accounts are by invitation only")).toBeVisible();
  await expect(page.getByText("Allowed domains", { exact: true })).toHaveCount(0);
});
