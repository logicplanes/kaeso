import type { Browser, Page } from "@playwright/test";
import {
  expect,
  test,
  NEW_PASSWORD,
  clearMail,
  expectSignedIn,
  mailLink,
  openDefaultProject,
  signInAsOwner,
  signInWithPassword,
  submitSteps,
  toastText,
} from "./support";

test.describe.configure({ mode: "serial" });

const NOOR = { name: "Noor Haddad", email: "noor@example.com" };
const ROLE = "Log reader";

const orgOf = (page: Page) => /\/orgs\/([^/]+)/.exec(new URL(page.url()).pathname)?.[1] ?? "";
const gridRow = (page: Page, grid: string, text: string) =>
  page
    .getByRole("grid", { name: grid, exact: true })
    .getByRole("gridcell")
    .filter({ has: page.getByText(text, { exact: true }) });

async function openSettings(page: Page, path: string): Promise<string> {
  await signInAsOwner(page);
  const org = orgOf(page);
  await page.goto(`/settings/orgs/${org}/${path}`);
  return org;
}

async function signInAsNoor(browser: Browser): Promise<Page> {
  const context = await browser.newContext();
  const page = await context.newPage();
  await signInWithPassword(page, NOOR.email, NEW_PASSWORD);
  await expectSignedIn(page);
  return page;
}

test("built-in roles are read-only, with their rights and who has them", async ({ page }) => {
  await openSettings(page, "members?tab=roles");
  await expect(page.getByRole("tab", { name: "Roles", selected: true })).toBeVisible();
  await gridRow(page, "Organization roles", "Auditor").click();
  const sheet = page.getByRole("dialog", { name: "Auditor" });
  await expect(sheet.getByText("Built-in", { exact: true })).toBeVisible();
  await expect(sheet.getByRole("checkbox")).toHaveCount(0);
  await expect(sheet.getByRole("group", { name: "Rights of Auditor" }).getByRole("img", { name: "Granted" }).first()).toBeVisible();
  await expect(sheet.getByRole("button", { name: "Save role" })).toHaveCount(0);
  await expect(sheet.getByRole("button", { name: "Duplicate" })).toBeEnabled();
});

test("an admin can't create a role with a right they don't hold", async ({ page }) => {
  await openSettings(page, "members?tab=roles");
  await page.getByRole("button", { name: "Create role" }).first().click();
  const dialog = page.getByRole("dialog", { name: "Create role" });
  await dialog.getByRole("textbox", { name: /^Name/ }).fill("Billing lead");
  await dialog.getByRole("radio", { name: /^Organization role/ }).click();
  await dialog.getByRole("button", { name: "Continue", exact: true }).click();
  await expect(dialog.getByRole("checkbox", { name: "Manage billing", exact: true })).toHaveAttribute("aria-disabled", "true");
  await page.keyboard.press("Escape");
});

test("a custom project role is created, with what its rights need", async ({ page }) => {
  await openSettings(page, "members?tab=roles");
  await page.getByRole("button", { name: "Create role" }).first().click();
  const dialog = page.getByRole("dialog", { name: "Create role" });
  await dialog.getByRole("textbox", { name: /^Name/ }).fill(ROLE);
  await dialog.getByRole("textbox", { name: /^Description/ }).fill("Reads request logs to help customers");
  await dialog.getByRole("radio", { name: /^Project role/ }).click();
  await dialog.getByRole("button", { name: "Continue", exact: true }).click();
  await dialog.getByRole("checkbox", { name: "View request logs", exact: true }).click();
  await expect(dialog.getByRole("checkbox", { name: "View project", exact: true })).toBeChecked();
  await submitSteps(dialog, "Create role");
  await expect(toastText(page, `${ROLE} created`)).toBeVisible();
  const sheet = page.getByRole("dialog", { name: ROLE });
  await expect(sheet.getByText("Custom", { exact: true })).toBeVisible();
  await expect(sheet.getByText("Nobody yet")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(gridRow(page, "Project roles", ROLE)).toBeVisible();
});

test("a new member joins as Member and sees what a Viewer may not do, with who can give it", async ({ page, browser }) => {
  await clearMail();
  const org = await openSettings(page, "members");
  await page.getByRole("button", { name: "Invite member" }).first().click();
  const dialog = page.getByRole("dialog", { name: "Invite members" });
  await dialog.getByRole("combobox", { name: "Email addresses" }).fill(NOOR.email);
  await dialog.getByRole("combobox", { name: "Email addresses" }).press("Enter");
  await dialog.getByRole("button", { name: "Continue", exact: true }).click();
  await expect(dialog.getByRole("radio", { name: "Owner" })).toHaveAttribute("aria-disabled", "true");
  await expect(dialog.getByText("Only owners can make someone an owner")).toBeVisible();
  await expect(dialog.getByRole("radio", { name: "Member" })).toBeChecked();
  await submitSteps(dialog, "Send invitation");
  await expect(toastText(page, `Invitation sent to ${NOOR.email}`)).toBeVisible();

  const link = await mailLink(NOOR.email, /\/invite/);
  const context = await browser.newContext();
  const noor = await context.newPage();
  await noor.goto(link);
  await noor.getByLabel("Name").fill(NOOR.name);
  await noor.getByLabel("Password").fill(NEW_PASSWORD);
  await noor.getByRole("button", { name: "Create account and join" }).click();
  await expectSignedIn(noor);
  await noor.goto(`/orgs/${org}`);
  await openDefaultProject(noor);
  const nav = noor.getByRole("navigation", { name: "Console" });
  await expect(nav.getByRole("link", { name: "Request logs" })).toBeVisible();
  await expect(nav.getByRole("link", { name: "Playground" })).toHaveCount(0);
  await nav.getByRole("link", { name: "API keys" }).click();
  const create = noor.getByRole("button", { name: "Create key" }).first();
  await expect(create).toBeDisabled();
  await create.hover();
  await expect(noor.getByText(/^You need Create own keys; ask .+ for it\.$/)).toBeVisible();
  await context.close();
});

test("the role is given in the project, limits the member to it, and they see where each right comes from", async ({ page, browser }) => {
  await signInAsOwner(page);
  await openDefaultProject(page);
  const project = new URL(page.url()).pathname;
  await page.getByRole("navigation", { name: "Console" }).getByRole("link", { name: "Members" }).click();
  await gridRow(page, "Members", NOOR.name).click();
  const sheet = page.getByRole("dialog", { name: NOOR.name });
  await sheet.getByRole("combobox", { name: "Role" }).click();
  await page.getByRole("option", { name: new RegExp(`^${ROLE}`) }).click();
  await sheet.getByRole("button", { name: "Save role" }).click();
  await expect(toastText(page, `${NOOR.name} is ${ROLE} in this project now`)).toBeVisible();
  const access = sheet.getByRole("group", { name: `${NOOR.name}'s rights in this project` }).first();
  await expect(access.getByText(ROLE, { exact: true })).toBeVisible();
  await expect(access.getByText("View request logs")).toBeVisible();

  const noor = await signInAsNoor(browser);
  await noor.goto(project);
  const nav = noor.getByRole("navigation", { name: "Console" });
  await expect(nav.getByRole("link", { name: "Request logs" })).toBeVisible();
  await expect(nav.getByRole("link", { name: "API keys" })).toHaveCount(0);
  await expect(nav.getByRole("link", { name: "Models" })).toHaveCount(0);
  await noor.goto(`${project}/keys`);
  await expect(toastText(noor, /^You need View all keys; ask .+ for it\.$/)).toBeVisible();
  await expect(noor).not.toHaveURL(/\/keys$/);
  await noor.context().close();
});

test("an admin can't change the owner, and sees an org member's rights with their sources", async ({ page }) => {
  await openSettings(page, "members");
  await gridRow(page, "Members", "Lin Park").click();
  const owner = page.getByRole("dialog", { name: "Lin Park" });
  await expect(owner.getByRole("combobox", { name: "Role" })).toBeDisabled();
  await expect(owner.getByRole("button", { name: "Remove from organization" })).toBeDisabled();
  await page.keyboard.press("Escape");

  await gridRow(page, "Members", NOOR.name).click();
  const sheet = page.getByRole("dialog", { name: NOOR.name });
  const access = sheet.getByRole("group", { name: `${NOOR.name}'s rights in the organization` }).first();
  await expect(access.getByText("Member", { exact: true })).toBeVisible();
  await sheet
    .getByRole("button", { name: /^Every right/ })
    .first()
    .click();
  const every = sheet.getByRole("group", { name: `${NOOR.name}'s rights in the organization` }).last();
  await expect(every.getByText("View members", { exact: true })).toBeVisible();
  await expect(sheet.getByRole("grid", { name: `${NOOR.name}'s projects` }).getByText(ROLE, { exact: true })).toBeVisible();
});

test("the role changes for everyone who has it, can't be deleted while held, then is", async ({ page }) => {
  await openSettings(page, "members?tab=roles");
  await gridRow(page, "Project roles", ROLE).click();
  const sheet = page.getByRole("dialog", { name: ROLE });
  await expect(sheet.getByRole("button", { name: "Delete role" })).toBeDisabled();
  await sheet.getByRole("checkbox", { name: "View usage", exact: true }).click();
  await sheet.getByRole("button", { name: "Save role" }).click();
  const confirm = page.getByRole("alertdialog");
  await expect(confirm).toContainText(`Change ${ROLE} for 1 member or service account?`);
  await confirm.getByRole("button", { name: "Save role" }).click();
  await expect(toastText(page, `${ROLE} saved for 1 member or service account`)).toBeVisible();

  await page.goto(`/orgs/${orgOf(page)}`);
  await openDefaultProject(page);
  await page.getByRole("navigation", { name: "Console" }).getByRole("link", { name: "Members" }).click();
  await gridRow(page, "Members", NOOR.name).click();
  const member = page.getByRole("dialog", { name: NOOR.name });
  await member.getByRole("combobox", { name: "Role" }).click();
  await page.getByRole("option", { name: /^Viewer/ }).click();
  await member.getByRole("button", { name: "Save role" }).click();
  await expect(toastText(page, `${NOOR.name} is Viewer in this project now`)).toBeVisible();

  await page.goto(`/settings/orgs/${orgOf(page)}/members?tab=roles`);
  await gridRow(page, "Project roles", ROLE).click();
  await page.getByRole("dialog", { name: ROLE }).getByRole("button", { name: "Delete role" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Delete role" }).click();
  await expect(toastText(page, `${ROLE} deleted`)).toBeVisible();
  await expect(gridRow(page, "Project roles", ROLE)).toHaveCount(0);
});
