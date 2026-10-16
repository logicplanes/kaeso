import type { Locator, Page } from "@playwright/test";
import { abstractPicture } from "../scripts/png.mjs";
import { expect, test, signInToProject, toastText } from "./support";

test.describe.configure({ mode: "serial" });

const orgOf = (page: Page) => /\/orgs\/([^/]+)/.exec(new URL(page.url()).pathname)?.[1] ?? "";

test("rename and content logging; provider keys are the organization's alone", async ({ page }) => {
  await signInToProject(page);
  await page.getByRole("link", { name: "Settings", exact: true }).click();
  await expect(page).toHaveTitle(/^Settings · /);

  const name = page.getByRole("textbox", { name: "Project name" });
  const before = await name.inputValue();
  await name.fill("Checkout");
  await name.press("Enter");
  await expect(page.getByText("Project renamed")).toBeVisible();
  await expect(page.getByRole("navigation", { name: "Console" }).getByText("Checkout", { exact: true })).toBeVisible();
  await name.fill(before);
  await name.press("Enter");
  await expect(page.getByRole("navigation", { name: "Console" }).getByText(before, { exact: true })).toBeVisible();

  const logging = page.getByRole("switch", { name: "Log request content" });
  await expect(logging).toBeChecked();
  await logging.click();
  await expect(page.getByText("Content logging turned off")).toBeVisible();
  await expect(logging).not.toBeChecked();
  await logging.click();
  await expect(page.getByText("Content logging turned on")).toBeVisible();

  await expect(page.getByText("Provider keys", { exact: true })).toHaveCount(0);
  await expect(page.getByRole("button", { name: /override/i })).toHaveCount(0);
});

test("the default project can't be archived; another one is, after typing its name", async ({ page }) => {
  await signInToProject(page);
  await page.getByRole("link", { name: "Settings", exact: true }).click();
  await expect(page.getByRole("button", { name: "Archive project" })).toBeDisabled();

  await page.goto(`/orgs/${orgOf(page)}/projects?create=true`);
  const create = page.getByRole("dialog", { name: "Create project" });
  await create.getByLabel("Name").fill("Short-lived");
  await create.getByRole("button", { name: "Create project" }).click();
  await expect(page.getByRole("navigation", { name: "Console" }).getByText("Short-lived", { exact: true })).toBeVisible();
  await page.getByRole("link", { name: "Settings", exact: true }).click();
  await page.getByRole("button", { name: "Archive project" }).click();
  const confirm = page.getByRole("alertdialog", { name: "Archive Short-lived?" });
  await expect(confirm.getByRole("button", { name: "Archive project" })).toBeDisabled();
  await confirm.getByRole("textbox").fill("Short-lived");
  await confirm.getByRole("button", { name: "Archive project" }).click();
  await expect(page.getByText("Short-lived archived")).toBeVisible();
  await expect(page.getByRole("link", { name: /Short-lived/ })).toHaveCount(0);

  await page.goto(`/orgs/${orgOf(page)}/projects`);
  await page.getByRole("tab", { name: "All", exact: true }).click();
  const archived = page.getByRole("gridcell").filter({ hasText: "Short-lived" });
  await expect(archived.getByText("Archived", { exact: true })).toBeVisible();
  await archived.click();
  await expect(page).toHaveURL(/\/orgs\/[^/]+\/projects\/[^/]+$/);
});

test("a project's picture is uploaded, shown beside its name, and removed", async ({ page }) => {
  await signInToProject(page);
  await page.getByRole("link", { name: "Settings", exact: true }).click();
  const sidebar = page.getByRole("navigation", { name: "Console" });
  const picture = (where: Locator) => where.locator('img[src*="/platform/v1/me/avatars/"]');
  await expect(page.getByRole("button", { name: "Upload", exact: true })).toBeVisible();
  await expect(picture(sidebar)).toHaveCount(0);

  await page.locator('input[type="file"]').setInputFiles({ name: "project.png", mimeType: "image/png", buffer: abstractPicture(3, 96) });
  await expect(toastText(page, "Picture saved")).toBeVisible();
  await expect(page.getByRole("button", { name: "Replace", exact: true })).toBeVisible();
  await expect(picture(sidebar)).toHaveCount(1);
  await page.goto(`/orgs/${orgOf(page)}/projects`);
  const row = page
    .getByRole("grid", { name: "Projects" })
    .getByRole("gridcell")
    .filter({ has: page.getByText("Default", { exact: true }) });
  await expect(picture(row)).toHaveCount(1);
  await expect(picture(sidebar)).toHaveCount(1);

  await row.click();
  await page.getByRole("link", { name: "Settings", exact: true }).click();
  await page.locator('input[type="file"]').setInputFiles({ name: "notes.txt", mimeType: "text/plain", buffer: Buffer.from("not a picture") });
  await expect(toastText(page, "Choose a PNG, JPEG or WebP image.")).toBeVisible();

  await page.getByRole("button", { name: "Remove", exact: true }).click();
  await expect(toastText(page, "Picture removed")).toBeVisible();
  await expect(page.getByRole("button", { name: "Upload", exact: true })).toBeVisible();
  await expect(picture(sidebar)).toHaveCount(0);
});
