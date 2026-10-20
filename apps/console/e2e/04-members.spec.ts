import type { Page } from "@playwright/test";
import { expect, test, NEW_PASSWORD, OWNER, clearMail, expectSignedIn, mailLink, signInAsOwner, signInToProject, submitSteps, toastText } from "./support";

test.describe.configure({ mode: "serial" });

const DEVELOPER = { name: "Lin Park", email: "developer@example.com" };

const row = (page: Page, grid: string, text: string) =>
  page
    .getByRole("grid", { name: grid, exact: true })
    .getByRole("gridcell")
    .filter({ has: page.getByText(text, { exact: true }) });

const orgOf = (page: Page) => /\/orgs\/([^/]+)/.exec(new URL(page.url()).pathname)?.[1] ?? "";

test("an invited person accepts and joins the organization", async ({ page, browser }) => {
  await clearMail();
  await signInAsOwner(page);
  await page.goto(`/settings/orgs/${orgOf(page)}/members?tab=invites`);
  await page.getByRole("button", { name: "Invite members" }).click();
  const dialog = page.getByRole("dialog", { name: "Invite members" });
  await dialog.getByRole("textbox", { name: "Email addresses" }).fill(DEVELOPER.email);
  await dialog.getByRole("textbox", { name: "Email addresses" }).press("Enter");
  await submitSteps(dialog, "Send invitation");
  await expect(toastText(page, `Invitation sent to ${DEVELOPER.email}`)).toBeVisible();

  const link = await mailLink(DEVELOPER.email, /\/invite/);
  const other = await browser.newContext();
  const invitee = await other.newPage();
  await invitee.goto(link);
  await expect(invitee).toHaveURL(/\/invite$/);
  await invitee.getByLabel("Name").fill(DEVELOPER.name);
  await invitee.getByLabel("Password", { exact: true }).fill(NEW_PASSWORD);
  await invitee.getByRole("button", { name: "Create account and join" }).click();
  await expectSignedIn(invitee);
  await expect(invitee.getByRole("button", { name: `${OWNER.org}: switch or open settings` })).toBeVisible();
  await other.close();
});

test("a member's rights are customized and their models limited; removed, then added back", async ({ page }) => {
  await signInToProject(page);
  await page.getByRole("link", { name: "Members" }).click();
  await expect(page).toHaveTitle(/^Members · /);
  await row(page, "Members", DEVELOPER.name).click();
  const sheet = page.getByRole("dialog", { name: DEVELOPER.name });
  await sheet.getByRole("button", { name: "Customize" }).click();
  await sheet.getByRole("checkbox", { name: /^(logs\.export|Export logs)/ }).click();
  await sheet.getByRole("button", { name: "Save role" }).click();
  await expect(toastText(page, `${DEVELOPER.name} is Viewer in this project now`)).toBeVisible();
  await expect(row(page, "Members", DEVELOPER.name).getByText("Viewer · customized", { exact: true })).toBeVisible();

  await sheet.getByRole("button", { name: "Edit", exact: true }).click();
  await sheet.getByRole("button", { name: "Models" }).click();
  await page.getByRole("option", { name: /^GPT-5\.6/ }).click();
  await page.keyboard.press("Escape");
  await sheet.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByText("Models saved")).toBeVisible();
  await expect(sheet.getByText("GPT-5.6", { exact: true })).toBeVisible();

  await sheet.getByRole("button", { name: "Remove from project" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Remove from project" }).click();
  await expect(page.getByText("Removed from the project")).toBeVisible();
  await expect(row(page, "Members", DEVELOPER.name)).toHaveCount(0);

  await page.getByRole("button", { name: "Add member" }).first().click();
  const dialog = page.getByRole("dialog", { name: "Add member" });
  await dialog.getByRole("combobox", { name: "Member" }).click();
  await page.getByRole("option", { name: new RegExp(DEVELOPER.name) }).click();
  await dialog.getByRole("combobox", { name: "Role" }).click();
  await page.getByRole("option", { name: /^Viewer/ }).click();
  await dialog.getByRole("button", { name: "Add member" }).click();
  await expect(page.getByText(`${DEVELOPER.name} added to the project`)).toBeVisible();
  await expect(row(page, "Members", DEVELOPER.name).getByText("Viewer", { exact: true })).toBeVisible();
});

test("a service account gets a key of its own and is archived with it", async ({ page }) => {
  await signInToProject(page);
  await page.getByRole("link", { name: "Service accounts" }).click();
  await expect(page.getByText("No service accounts yet")).toBeVisible();
  await page.getByRole("button", { name: "Create service account" }).first().click();
  const dialog = page.getByRole("dialog", { name: "Create service account" });
  await dialog.getByLabel("Name").fill("deploy-bot");
  await submitSteps(dialog, "Create service account");
  await expect(page.getByText("Service account deploy-bot created")).toBeVisible();

  const sheet = page.getByRole("dialog", { name: "deploy-bot" });
  await sheet.getByRole("button", { name: "Create key" }).click();
  const create = page.getByRole("dialog", { name: "Create key" });
  await expect(create.getByRole("heading", { name: "A key for deploy-bot" })).toBeVisible();
  await create.getByLabel("Name").fill("deploy-bot-production");
  await submitSteps(create, "Create key");
  await page.getByRole("dialog", { name: "Key created" }).getByRole("button", { name: "Done" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Close without copying" }).click();
  await expect(row(page, "Keys of deploy-bot", "deploy-bot-production")).toBeVisible();
  await expect(row(page, "Service accounts", "deploy-bot").getByText("1 key")).toBeVisible();

  await sheet.getByRole("button", { name: "Archive service account" }).click();
  const confirm = page.getByRole("alertdialog");
  await expect(confirm.getByText("Its keys are archived with it")).toBeVisible();
  await confirm.getByRole("button", { name: "Archive service account" }).click();
  await expect(page.getByText("Service account archived")).toBeVisible();
  await expect(row(page, "Service accounts", "deploy-bot")).toHaveCount(0);
  await page.getByRole("tablist", { name: "Service accounts" }).getByRole("tab", { name: "All", exact: true }).click();
  await expect(row(page, "Service accounts", "deploy-bot").getByText("Archived")).toBeVisible();
});
