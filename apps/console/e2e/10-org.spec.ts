import type { Page } from "@playwright/test";
import {
  expect,
  test,
  NEW_PASSWORD,
  OWNER,
  expectSignedIn,
  signInAsOwner,
  signInWithPassword,
  stepTo,
  submitSteps,
  toastText,
  withoutSaveDialog,
} from "./support";

test.describe.configure({ mode: "serial" });

async function openOrgSettings(page: Page, label: string) {
  await signInAsOwner(page);
  await page.getByRole("button", { name: `${OWNER.org}: switch or open settings` }).click();
  await page.getByRole("menuitem", { name: "Organization settings" }).click();
  await expect(page).toHaveURL(/\/settings\/orgs\/[^/]+\/general$/);
  if (label !== "General") await page.locator(`nav[aria-label="Settings"] a[href^="/settings/orgs/"]`).getByText(label, { exact: true }).click();
  await expect(page.getByRole("heading", { level: 1, name: label })).toBeVisible();
}

test("general: the organization is renamed and renamed back", async ({ page }) => {
  await openOrgSettings(page, "General");
  const name = page.getByRole("textbox", { name: "Organization name" });
  await name.fill("Northwind");
  await name.press("Enter");
  await expect(toastText(page, "Name saved")).toBeVisible();
  await expect(page.getByRole("navigation", { name: "Settings" }).getByText("Northwind", { exact: true })).toBeVisible();
  await name.fill(OWNER.org);
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByRole("navigation", { name: "Settings" }).getByText(OWNER.org, { exact: true })).toBeVisible();
});

test("members: a member's organization role changes in the sheet", async ({ page }) => {
  await openOrgSettings(page, "Members");
  await page.getByRole("gridcell").filter({ hasText: "Lin Park" }).first().click();
  const sheet = page.getByRole("dialog", { name: "Lin Park" });
  await sheet.getByRole("combobox", { name: "Role" }).click();
  await page.getByRole("option", { name: /^Developer/ }).click();
  await sheet.getByRole("button", { name: "Save role" }).click();
  await expect(toastText(page, "Lin Park is Developer now")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("gridcell").filter({ hasText: "Lin Park" }).getByText("Developer", { exact: true })).toBeVisible();
});

test("invites: an invitation is sent, resent and revoked on the Members page's Invites tab", async ({ page }) => {
  await openOrgSettings(page, "Members");
  await expect(page.locator(`nav[aria-label="Settings"] a[href^="/settings/orgs/"]`).getByText("Invites", { exact: true })).toHaveCount(0);
  await page.getByRole("tab", { name: "Invites", exact: true }).click();
  await expect(page).toHaveURL(/\/settings\/orgs\/[^/]+\/members\?tab=invites$/);
  await page.getByRole("button", { name: "Invite member" }).click();
  const dialog = page.getByRole("dialog", { name: "Invite members" });
  const emails = dialog.getByRole("combobox", { name: "Email addresses" });
  await emails.fill("temp@example.com");
  await emails.press("Enter");
  await expect(dialog.getByText("temp@example.com")).toBeVisible();
  await submitSteps(dialog, "Send invitation");
  await expect(toastText(page, "Invitation sent to temp@example.com")).toBeVisible();

  const row = page.getByRole("gridcell").filter({ hasText: "temp@example.com" });
  await row.getByRole("button", { name: "Actions for the invitation to temp@example.com" }).click();
  await page.getByRole("menuitem", { name: "Resend" }).click();
  await expect(toastText(page, "Invitation sent to temp@example.com again")).toBeVisible();
  await row.getByRole("button", { name: "Actions for the invitation to temp@example.com" }).click();
  await page.getByRole("menuitem", { name: "Revoke" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Revoke invitation" }).click();
  await expect(toastText(page, "Invitation revoked")).toBeVisible();
  await expect(row).toHaveCount(0);
  await page.getByRole("tab", { name: "All", exact: true }).click();
  await expect(page.getByRole("gridcell").filter({ hasText: "temp@example.com" }).getByText("Revoked")).toBeVisible();

  const org = /\/settings\/orgs\/([^/]+)/.exec(new URL(page.url()).pathname)?.[1] ?? "";
  await page.goto(`/settings/orgs/${org}/invites?invite=true`);
  await expect(page.getByRole("dialog", { name: "Invite members" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog", { name: "Invite members" })).toBeHidden();
  await expect(page.getByRole("heading", { level: 1, name: "Members" })).toBeVisible();
  await expect(page.getByRole("tab", { name: "Invites", exact: true })).toHaveAttribute("aria-selected", "true");
});

test("invites: the dialog opens empty on its first step after a cancelled or a sent invitation", async ({ page }) => {
  await openOrgSettings(page, "Members");
  await page.getByRole("tab", { name: "Invites", exact: true }).click();
  const invite = page.getByRole("button", { name: "Invite member" });
  const dialog = page.getByRole("dialog", { name: "Invite members" });
  const emails = dialog.getByRole("combobox", { name: "Email addresses" });
  await invite.click();
  await emails.fill("fresh@example.com");
  await emails.press("Enter");
  await expect(dialog.getByText("fresh@example.com")).toBeVisible();
  await stepTo(dialog, dialog.getByRole("heading", { name: "What can they do in the organization?" }));
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();

  await invite.click();
  await expect(dialog.getByText("Step 1 of 3")).toBeVisible();
  await expect(emails).toHaveValue("");
  await expect(dialog.getByText("fresh@example.com")).toHaveCount(0);
  await emails.fill("fresh@example.com");
  await emails.press("Enter");
  await submitSteps(dialog, "Send invitation");
  await expect(toastText(page, "Invitation sent to fresh@example.com")).toBeVisible();
  await expect(dialog).toBeHidden();

  await invite.click();
  await expect(dialog.getByText("Step 1 of 3")).toBeVisible();
  await expect(emails).toHaveValue("");
  await expect(dialog.getByText("fresh@example.com")).toHaveCount(0);
  await expect(dialog.getByRole("button", { name: "Continue", exact: true })).toHaveAttribute("aria-disabled", "true");
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();

  const row = page.getByRole("gridcell").filter({ hasText: "fresh@example.com" });
  await row.getByRole("button", { name: "Actions for the invitation to fresh@example.com" }).click();
  await page.getByRole("menuitem", { name: "Revoke" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Revoke invitation" }).click();
  await expect(toastText(page, "Invitation revoked")).toBeVisible();
});

test("projects: the settings address opens the organization's Projects page; a project is created there", async ({ page }) => {
  await openOrgSettings(page, "General");
  await expect(page.locator(`nav[aria-label="Settings"] a[href^="/settings/orgs/"]`).getByText("Projects", { exact: true })).toHaveCount(0);
  const org = /\/settings\/orgs\/([^/]+)/.exec(new URL(page.url()).pathname)?.[1] ?? "";
  await page.goto(`/settings/orgs/${org}/projects`);
  await expect(page).toHaveURL(new RegExp(`/orgs/${org}/projects$`));
  await expect(page.getByRole("heading", { level: 1, name: "Projects" })).toBeVisible();
  await page.getByRole("button", { name: "Create project" }).click();
  const dialog = page.getByRole("dialog", { name: "Create project" });
  await dialog.getByLabel("Name").fill("Research");
  await dialog.getByRole("button", { name: "Create project" }).click();
  await expect(toastText(page, "Research created")).toBeVisible();
  await expect(page).toHaveURL(/\/orgs\/[^/]+\/projects\/[^/]+$/);
  await expect(page.getByRole("navigation", { name: "Console" }).getByText("Research", { exact: true })).toBeVisible();
});

test("projects: an admin makes Research the default and back after a confirmation; a developer can't", async ({ page, browser }) => {
  await openOrgSettings(page, "General");
  const org = /\/settings\/orgs\/([^/]+)/.exec(new URL(page.url()).pathname)?.[1] ?? "";
  await page.goto(`/orgs/${org}/projects`);
  const grid = page.getByRole("grid", { name: "Projects" });
  const row = (name: string) => grid.getByRole("gridcell").filter({ hasText: name });
  const actions = page.getByRole("button", { name: /^Actions for / });
  await expect(actions).toHaveCount(1);
  await expect(actions).toHaveAccessibleName("Actions for Research");
  const listed = (await (await page.request.get(`/platform/v1/orgs/${org}/projects`)).json()) as { data: { name: string; is_default: boolean }[] };
  const before = listed.data.find((p) => p.is_default)?.name ?? "";

  await row("Research").hover();
  await page.getByRole("button", { name: "Actions for Research" }).click();
  await page.getByRole("menuitem", { name: "Make default" }).click();
  const confirm = page.getByRole("dialog", { name: "Make default project" });
  await expect(confirm.getByText("Make Research the default?")).toBeVisible();
  await expect(confirm.getByText(`${before} stays a project; its members keep their places.`)).toBeVisible();
  await confirm.getByRole("button", { name: "Make default" }).click();
  await expect(toastText(page, "Research is the default project")).toBeVisible();
  await expect(row("Research").getByText("Default", { exact: true })).toBeVisible();
  await expect(row(before).getByText("Default", { exact: true })).toHaveCount(0);
  await expect(page.getByRole("button", { name: `Actions for ${before}` })).toBeVisible();

  const other = await browser.newContext();
  const developer = await other.newPage();
  await signInWithPassword(developer, "developer@example.com", NEW_PASSWORD);
  await expectSignedIn(developer);
  await developer.goto(`/orgs/${org}/projects`);
  await expect(developer.getByRole("grid", { name: "Projects" }).getByText("Research", { exact: true })).toBeVisible();
  await expect(developer.getByRole("button", { name: /^Actions for / })).toHaveCount(0);
  await other.close();

  await row(before).hover();
  await page.getByRole("button", { name: `Actions for ${before}` }).click();
  await page.getByRole("menuitem", { name: "Make default" }).click();
  await page.getByRole("dialog", { name: "Make default project" }).getByRole("button", { name: "Make default" }).click();
  await expect(toastText(page, `${before} is the default project`)).toBeVisible();
  await expect(row(before).getByText("Default", { exact: true })).toBeVisible();
  await expect(row("Research").getByText("Default", { exact: true })).toHaveCount(0);
});

test("provider keys: several per provider, one default, assigned to a project, replaced, renamed and deleted", async ({ page }) => {
  await openOrgSettings(page, "General");
  const org = /\/settings\/orgs\/([^/]+)/.exec(new URL(page.url()).pathname)?.[1] ?? "";
  await page.goto(`/settings/orgs/${org}/provider-keys`);
  await expect(page).toHaveURL(new RegExp(`/orgs/${org}/providers$`));
  await expect(page.getByRole("heading", { level: 1, name: "Providers" })).toBeVisible();

  for (const [name, secret] of [
    ["Research", "sk-ant-mock-11111111111111anthropic"],
    ["Batch jobs", "sk-ant-mock-33333333333333anthr0p1x"],
  ] as const) {
    await page.getByRole("button", { name: "Add key", exact: true }).click();
    const add = page.getByRole("dialog", { name: "Add provider key" });
    await add.getByRole("radio", { name: /Anthropic/ }).click();
    await add.getByRole("button", { name: "Continue" }).click();
    await add.getByRole("textbox", { name: "Anthropic API key", exact: true }).fill(secret);
    await add.getByLabel("Name", { exact: true }).fill(name);
    const submit = add.getByRole("button", { name: "Add key" });
    if (!(await submit.isVisible())) await add.getByRole("button", { name: "Continue" }).click();
    await submit.click();
    await expect(toastText(page, new RegExp(`^${name} (added|is the Anthropic default)$`))).toBeVisible();
  }
  const row = (name: string) => page.getByRole("row").filter({ hasText: name });

  await page.getByRole("button", { name: "Actions for Batch jobs" }).click();
  await page.getByRole("menuitem", { name: "Make default" }).click();
  await expect(toastText(page, "Batch jobs is the Anthropic default")).toBeVisible();
  await expect(row("Batch jobs").getByText("Default", { exact: true })).toBeVisible();
  await expect(row("Research").getByText("Default", { exact: true })).toHaveCount(0);

  await page.getByRole("button", { name: "Actions for Research" }).click();
  await page.getByRole("menuitem", { name: "Assign to projects" }).click();
  const assign = page.getByRole("dialog", { name: "Assign Research" });
  await assign.getByRole("checkbox").first().click();
  await assign.getByRole("button", { name: "Save" }).click();
  await expect(toastText(page, "Research assigned to 1 project")).toBeVisible();

  await page.getByRole("button", { name: "Actions for Research" }).click();
  await page.getByRole("menuitem", { name: "Replace" }).click();
  const replace = page.getByRole("dialog", { name: "Replace Research" });
  await replace.getByLabel("Anthropic API key").fill("  sk-ant-mock-22222222222222anthr0p1c  ");
  await replace.getByRole("button", { name: "Replace key" }).click();
  await expect(toastText(page, "Research replaced")).toBeVisible();
  await expect(page.getByText("…0p1c")).toBeVisible();

  await page.getByRole("button", { name: "Actions for Research" }).click();
  await page.getByRole("menuitem", { name: "Rename" }).click();
  const rename = page.getByRole("dialog", { name: "Rename Research" });
  await rename.getByLabel("Name").fill("Research v2");
  await rename.getByRole("button", { name: "Rename" }).click();
  await expect(toastText(page, "Renamed to Research v2")).toBeVisible();

  await page.getByRole("button", { name: "Actions for Research v2" }).click();
  await page.getByRole("menuitem", { name: "Delete" }).click();
  const confirm = page.getByRole("alertdialog", { name: "Delete Research v2?" });
  await expect(confirm.getByText("Its projects switch to the Anthropic default.")).toBeVisible();
  await confirm.getByRole("button", { name: "Delete key" }).click();
  await expect(toastText(page, "Research v2 deleted")).toBeVisible();
  await expect(page.getByText("…0p1c")).toHaveCount(0);
});

test("security: the maximum key lifetime is set; requiring two-factor authentication without it asks first", async ({ page }) => {
  await openOrgSettings(page, "Security");
  await expect(page.getByRole("switch", { name: "Require single sign-on" })).toHaveAttribute("aria-disabled", "true");

  await page.getByRole("switch", { name: "No limit" }).click();
  await expect(toastText(page, "Key lifetime saved")).toBeVisible();
  const days = page.getByRole("textbox", { name: "Maximum key lifetime" });
  await days.fill("30");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByText("New keys expire within 30 days at most.")).toBeVisible();
  await page.getByRole("switch", { name: "No limit" }).click();
  await expect(page.getByText("Keys may never expire.")).toBeVisible();

  const tfa = page.getByRole("switch", { name: "Require two-factor authentication" });
  await tfa.click();
  const confirm = page.getByRole("alertdialog", { name: "Require two-factor authentication?" });
  await expect(confirm.getByText(/You don't use it yet/)).toBeVisible();
  await confirm.getByRole("button", { name: "Cancel" }).click();
  await expect(tfa).not.toBeChecked();
});

test("platform keys: one is created and shown once, calls the Platform API, and is archived", async ({ page, request }) => {
  await openOrgSettings(page, "Platform keys");
  const org = /\/settings\/orgs\/([^/]+)/.exec(new URL(page.url()).pathname)?.[1] ?? "";
  await page.getByRole("button", { name: "Create platform key" }).click();
  const dialog = page.getByRole("dialog", { name: "Create platform key" });
  await dialog.getByLabel("Name").fill("Terraform");
  await submitSteps(dialog, "Create key");
  const reveal = page.getByRole("dialog", { name: "Platform key created" });
  const secret = (await reveal.getByRole("textbox", { name: "Secret for Terraform" }).textContent()) ?? "";
  expect(secret).toMatch(/^sk-kso-org-/);
  await expect(reveal.getByText("Authorization: Bearer")).toBeVisible();
  await reveal.getByRole("button", { name: "Copy key" }).click();
  await reveal.getByRole("button", { name: "Done" }).click();

  const res = await request.get(`/platform/v1/orgs/${org}/projects`, { headers: { authorization: `Bearer ${secret}` } });
  expect(res.status()).toBe(200);

  const row = page.getByRole("gridcell").filter({ hasText: "Terraform" });
  await row.getByRole("button", { name: "Actions for Terraform" }).click();
  await page.getByRole("menuitem", { name: "Archive" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Archive key" }).click();
  await expect(toastText(page, "Terraform archived")).toBeVisible();
  expect((await request.get(`/platform/v1/orgs/${org}/projects`, { headers: { authorization: `Bearer ${secret}` } })).status()).toBe(401);
});

test("logging: retention is shortened, storage is listed, and a deletion is queued", async ({ page }) => {
  await openOrgSettings(page, "Logging");
  const content = page.getByRole("textbox", { name: "Content retention" });
  await content.fill("7");
  await page.getByRole("button", { name: "Save retention" }).click();
  await expect(toastText(page, "Retention saved")).toBeVisible();

  await page.getByRole("tab", { name: "Storage" }).click();
  await expect(page.getByText("Back up the master keys")).toBeVisible();

  await page.getByRole("tab", { name: "Deletions" }).click();
  await page.getByRole("button", { name: "Delete logs" }).click();
  const dialog = page.getByRole("dialog", { name: "Delete request logs" });
  await dialog.getByRole("button", { name: "Period" }).click();
  await page.getByRole("option", { name: "Last 7 days" }).click();
  const typed = dialog.getByRole("textbox", { name: "Type delete to confirm" });
  await stepTo(dialog, typed);
  await typed.fill("delete");
  await submitSteps(dialog, "Delete logs");
  await expect(toastText(page, "Deletion queued; it runs in the background")).toBeVisible();
  await expect(page.getByRole("table", { name: "Log deletions" }).getByText("Deletion")).toBeVisible();
});

test("audit log: the changes above are recorded, filtered by action, and open with what they changed", async ({ page }) => {
  await openOrgSettings(page, "Audit log");
  const grid = page.getByRole("table", { name: "Audit log" });
  await expect(grid.getByText("org.update").first()).toBeVisible();
  await page.getByRole("combobox", { name: "Action" }).fill("platform_key");
  await page.getByRole("option", { name: "platform_key.*" }).click();
  await expect(grid.getByText("platform_key.create")).toBeVisible();
  await expect(grid.getByText("org.update")).toHaveCount(0);
  await grid.getByText("platform_key.create").click();
  const sheet = page.getByRole("dialog", { name: "platform_key.create" });
  await expect(sheet.getByText("Ada Okafor (you)")).toBeVisible();
  await expect(sheet.getByText(/"name": "Terraform"/)).toBeVisible();
  await page.keyboard.press("Escape");

  await withoutSaveDialog(page);
  const download = page.waitForEvent("download");
  await page.getByRole("button", { name: "Export" }).click();
  const file = await download;
  await expect(toastText(page, "Export started")).toBeVisible();
  expect(file.suggestedFilename()).toMatch(/\.jsonl$/);
  const lines = (await (await file.createReadStream()).toArray().then((c) => Buffer.concat(c).toString("utf8"))).trim().split("\n");
  expect(lines.length).toBeGreaterThan(0);
  expect(lines.every((l) => (JSON.parse(l) as { action: string }).action.startsWith("platform_key."))).toBe(true);
});

test("usage: the old settings page opens the overview with its period", async ({ page }) => {
  await openOrgSettings(page, "General");
  await expect(page.locator(`nav[aria-label="Settings"] a[href^="/settings/orgs/"]`).getByText("Usage", { exact: true })).toHaveCount(0);
  const org = /\/settings\/orgs\/([^/]+)/.exec(new URL(page.url()).pathname)?.[1] ?? "";
  await page.goto(`/settings/orgs/${org}/usage?from=2026-01-01&to=2026-01-31`);
  await expect(page).toHaveURL(new RegExp(`/orgs/${org}\\?from=2026-01-01&to=2026-01-31$`));
  await expect(page.getByRole("heading", { level: 1, name: "Overview" })).toBeVisible();
  await expect(page.getByLabel("Usage in the period")).toBeVisible();
  await expect(
    page
      .getByRole("grid", { name: "Projects" })
      .getByText(/requests?$/)
      .first(),
  ).toBeVisible();
});
