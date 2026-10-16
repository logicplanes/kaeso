import { expect, test, signInToProject, stepTo, submitSteps } from "./support";

test.describe.configure({ mode: "serial" });

test("a project budget, a member limit above it is warned about, then edited and removed", async ({ page }) => {
  await signInToProject(page);
  await page.getByRole("link", { name: "Limits" }).click();
  await expect(page).toHaveTitle(/^Limits · /);
  await expect(page.getByText("No limits yet")).toBeVisible();

  await page.getByRole("button", { name: "Add limit" }).first().click();
  let dialog = page.getByRole("dialog", { name: "Add limit" });
  await stepTo(dialog, dialog.getByLabel("Amount"));
  await dialog.getByLabel("Amount").fill("50");
  await submitSteps(dialog, "Add limit");
  await expect(page.getByText("Added the monthly spend budget of 50 USD")).toBeVisible();
  await expect(page.getByText("$0.00 of $50.00")).toBeVisible();

  await page.getByRole("button", { name: "Add limit" }).first().click();
  dialog = page.getByRole("dialog", { name: "Add limit" });
  await dialog.getByRole("radio", { name: "Member" }).click();
  await dialog.getByRole("combobox", { name: "Member or service account" }).click();
  await page.getByRole("option", { name: /Lin Park/ }).click();
  await stepTo(dialog, dialog.getByLabel("Amount"));
  await dialog.getByLabel("Amount").fill("80");
  await expect(dialog.getByRole("button", { name: "Add limit" })).toBeDisabled();
  await dialog.getByLabel("Amount").fill("20");
  await expect(dialog.getByRole("button", { name: "Add limit" })).toBeEnabled();
  await dialog.getByRole("button", { name: "Add limit" }).click();
  await expect(page.getByText("Added the monthly spend budget of 20 USD")).toBeVisible();
  await expect(page.getByText("Lin Park", { exact: true })).toBeVisible();

  await page.getByRole("row").filter({ hasText: "Lin Park" }).click();
  const edit = page.getByRole("dialog", { name: "Edit limit" });
  await edit.getByLabel("Amount").fill("25");
  await edit.getByRole("button", { name: "Save limit" }).click();
  await expect(page.getByText("Limit saved")).toBeVisible();
  await expect(page.getByText("$0.00 of $25.00")).toBeVisible();

  await page.getByRole("row").filter({ hasText: "Lin Park" }).click();
  await page.getByRole("dialog", { name: "Edit limit" }).getByRole("button", { name: "Remove limit" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Remove limit" }).click();
  await expect(page.getByText("Limit removed")).toBeVisible();
  await expect(page.getByText("$0.00 of $25.00")).toHaveCount(0);
});
