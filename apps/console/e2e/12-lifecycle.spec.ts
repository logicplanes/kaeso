import type { Page } from "@playwright/test";
import { expect, test, NEW_PASSWORD, OWNER, openDefaultProject, signInAsOwner, signInWithPassword, stepTo, toastText } from "./support";

test.describe.configure({ mode: "serial" });

async function openGeneral(page: Page) {
  await signInAsOwner(page);
  await page.getByRole("button", { description: "Switch or open settings" }).click();
  await page.getByRole("menuitem", { name: "Organization settings" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "General" })).toBeVisible();
}

async function expectNoToasts(page: Page) {
  await page.waitForLoadState("networkidle");
  await expect(page.getByRole("region", { name: "Notifications" })).toHaveText("");
}

test("a scheduled organization stays readable without refused requests, sorts first for instance admins, and is restored", async ({ page }) => {
  await openGeneral(page);
  await page.getByRole("button", { name: "Delete organization" }).click();
  const dialog = page.getByRole("dialog", { name: `Delete ${OWNER.org}` });
  await expect(dialog.getByText(/it's deleted for good after \d+ days?\./)).toBeVisible();
  await expect(dialog.getByText("Members still see everything, read-only; an owner can restore it.")).toBeVisible();
  const name = dialog.getByRole("textbox", { name: "Organization name" });
  await stepTo(dialog, name);
  await expect(dialog.getByRole("button", { name: "Delete organization" })).toBeDisabled();
  await name.fill(OWNER.org);
  await dialog.getByRole("button", { name: "Delete organization" }).click();
  await expect(toastText(page, `${OWNER.org} is scheduled for deletion`)).toBeVisible();
  await expect(page.getByText("This organization is scheduled for deletion")).toBeVisible();

  await page.goto("/");
  await expect(page.getByRole("button", { description: "Switch or open settings" }).getByText(/^Deletion on /)).toBeVisible();
  await expect(page.getByText(/It's read-only and its keys stopped working; on .+ it's deleted for good\./)).toBeVisible();
  await expectNoToasts(page);
  await openDefaultProject(page);
  await page.getByRole("navigation", { name: "Console" }).getByRole("link", { name: "API keys" }).click();
  await expect(page.getByRole("button", { name: "Create key" }).first()).toBeDisabled();
  await expectNoToasts(page);
  const org = new URL(page.url()).pathname.split("/")[2] ?? "";
  for (const path of ["members", "security", "logging", "audit-log"]) {
    await page.goto(`/settings/orgs/${org}/${path}`);
    await expect(page.getByText("This organization is scheduled for deletion")).toBeVisible();
    await expectNoToasts(page);
  }

  await page.goto("/settings/instance/orgs");
  const state = page.getByRole("columnheader", { name: "State" });
  await expect(page.getByRole("table", { name: "Organizations" }).getByText(OWNER.org)).toBeVisible();
  await state.getByRole("button").click();
  await expect(page).toHaveURL(/\/settings\/instance\/orgs\?sort=deletion$/);
  await page.reload();
  await expect(state).toHaveAttribute("aria-sort", "ascending");
  await expect(page.getByRole("table", { name: "Organizations" }).getByRole("row").nth(1)).toContainText(OWNER.org);
  await expect(page.getByRole("table", { name: "Organizations" }).getByText(/^Deletion on /)).toBeVisible();
  await state.getByRole("button").click();
  await expect(page).toHaveURL(/\/settings\/instance\/orgs$/);

  await page.goto("/");
  await page.getByRole("button", { name: "Restore organization" }).click();
  await expect(toastText(page, `${OWNER.org} restored`)).toBeVisible();
  await expect(page.getByText("This organization is scheduled for deletion")).toHaveCount(0);
  await expect(page.getByRole("button", { description: "Switch or open settings" }).getByText(/^Deletion on /)).toHaveCount(0);
});

test("an instance admin deletes a scheduled organization now from the Organizations table", async ({ page }) => {
  const name = "Tailspin Toys";
  await signInAsOwner(page);
  await page.goto("/settings/instance/orgs");
  await page.getByRole("button", { name: "Create organization" }).click();
  const create = page.getByRole("dialog", { name: "Create organization" });
  await create.getByLabel("Name", { exact: true }).fill(name);
  await create.getByRole("button", { name: "Create organization" }).click();
  await expect(page.getByRole("heading", { level: 1, name })).toBeVisible();
  const org = new URL(page.url()).pathname.split("/").pop() ?? "";

  await page.goto(`/settings/orgs/${org}/general`);
  await page.getByRole("button", { name: "Delete organization" }).click();
  const schedule = page.getByRole("dialog", { name: `Delete ${name}` });
  const typed = schedule.getByRole("textbox", { name: "Organization name" });
  await stepTo(schedule, typed);
  await typed.fill(name);
  await schedule.getByRole("button", { name: "Delete organization" }).click();
  await expect(toastText(page, `${name} is scheduled for deletion`)).toBeVisible();

  await page.goto("/settings/instance/orgs?sort=deletion");
  const table = page.getByRole("table", { name: "Organizations" });
  await table.getByRole("row").filter({ hasText: name }).click({ button: "right" });
  await expect(page.getByRole("menuitem", { name: "Restore organization" })).toBeVisible();
  await page.getByRole("menuitem", { name: "Delete now" }).click();
  const dialog = page.getByRole("dialog", { name: `Delete ${name} now` });
  const confirm = dialog.getByRole("textbox", { name: "Organization name" });
  await stepTo(dialog, confirm);
  await expect(dialog.getByRole("button", { name: "Delete now" })).toBeDisabled();
  await confirm.fill(name);
  await dialog.getByRole("button", { name: "Delete now" }).click();
  const done = toastText(page, new RegExp(`^${name} (deleted|is deleted within minutes)$`));
  await expect(done).toBeVisible();
  await expect(page).toHaveURL(/\/settings\/instance\/orgs\?sort=deletion$/);
  if ((await done.textContent()) === `${name} deleted`) await expect(table.getByText(name)).toHaveCount(0);
});

test("an instance admin manages an organization from its instance page: rename, add an owner, schedule deletion and restore", async ({ page }) => {
  const name = "Northwind Traders";
  await signInAsOwner(page);
  await page.goto("/settings/instance/orgs");
  await page.getByRole("button", { name: "Create organization" }).click();
  const create = page.getByRole("dialog", { name: "Create organization" });
  await create.getByLabel("Name", { exact: true }).fill(name);
  await create.getByRole("button", { name: "Create organization" }).click();
  await expect(page.getByRole("heading", { level: 1, name })).toBeVisible();
  await expect(page.getByRole("link", { name: "Open organization" })).toBeVisible();

  await page.getByRole("button", { name: "Rename" }).click();
  const rename = page.getByRole("dialog", { name: "Rename the organization" });
  await rename.getByRole("textbox", { name: "Name" }).fill("Northwind Logistics");
  await rename.getByRole("button", { name: "Rename" }).click();
  await expect(toastText(page, "Renamed to Northwind Logistics")).toBeVisible();
  await expect(page.getByRole("heading", { level: 1, name: "Northwind Logistics" })).toBeVisible();

  await page.getByRole("button", { name: "Add owner" }).click();
  const add = page.getByRole("dialog", { name: "Add an owner" });
  await expect(add.getByRole("button", { name: "Add me" })).toHaveCount(0);
  await add.getByRole("combobox", { name: "User" }).fill("Lin");
  await page.getByRole("option", { name: /Lin Park/ }).click();
  await add.getByRole("button", { name: "Add owner" }).click();
  await expect(toastText(page, "Owner added")).toBeVisible();
  await expect(page.getByText("2 owners")).toBeVisible();

  await page.getByRole("button", { name: "Delete organization" }).click();
  const schedule = page.getByRole("dialog", { name: "Delete Northwind Logistics" });
  const typed = schedule.getByRole("textbox", { name: "Organization name" });
  await stepTo(schedule, typed);
  await typed.fill("Northwind Logistics");
  await schedule.getByRole("button", { name: "Delete organization" }).click();
  await expect(toastText(page, /^Northwind Logistics will be deleted on /)).toBeVisible();
  await expect(page.getByText(/^Deletion on /)).toBeVisible();
  await page.getByRole("button", { name: "Restore organization" }).click();
  await expect(toastText(page, "Northwind Logistics restored")).toBeVisible();
  await expect(page.getByRole("button", { name: "Delete organization" })).toBeVisible();
});

test("ownership is transferred: the other member becomes an owner and you an admin", async ({ page }) => {
  await openGeneral(page);
  await page.getByRole("button", { name: "Transfer ownership" }).click();
  const dialog = page.getByRole("dialog", { name: "Transfer ownership" });
  await dialog.getByRole("combobox", { name: "New owner" }).fill("Lin");
  await page.getByRole("option", { name: /Lin Park/ }).click();
  await expect(dialog.getByText("Lin Park becomes an owner and you become an admin.")).toBeVisible();
  await dialog.getByRole("button", { name: "Transfer ownership" }).click();
  await expect(toastText(page, "Lin Park is an owner now; you're an admin")).toBeVisible();

  await page.getByRole("navigation", { name: "Settings" }).getByRole("link", { name: "Members", exact: true }).click();
  const rows = page.getByRole("gridcell");
  await expect(rows.filter({ hasText: "Lin Park" }).getByText("Owner", { exact: true })).toBeVisible();
  await expect(rows.filter({ hasText: `${OWNER.name} (you)` }).getByText("Admin", { exact: true })).toBeVisible();
});

test("signing out everywhere ends the other sessions", async ({ page, browser }) => {
  const other = await browser.newContext();
  const elsewhere = await other.newPage();
  await signInWithPassword(elsewhere, OWNER.email, NEW_PASSWORD);
  await expect(elsewhere.getByRole("navigation", { name: "Console" })).toBeVisible();

  await signInAsOwner(page);
  await page.goto("/settings/account/sessions");
  await page.getByRole("button", { name: "Sign out everywhere" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Sign out everywhere" }).click();
  await expect(toastText(page, /Signed out of \d+ other sessions?/)).toBeVisible();

  await elsewhere.reload();
  await expect(elsewhere).toHaveURL(/\/sign-in/);
  await signInWithPassword(elsewhere, OWNER.email, NEW_PASSWORD);
  await expect(elsewhere.getByRole("navigation", { name: "Console" })).toBeVisible();
  await other.close();
});
