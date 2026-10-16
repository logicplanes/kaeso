import { expect, test, signInAsOwner } from "./support";

test("the palette finds a key, opens it, and lists it under Recent", async ({ page }) => {
  await signInAsOwner(page);
  await page.keyboard.press("ControlOrMeta+k");
  const palette = page.getByRole("dialog", { name: /command/i });
  await expect(palette.getByRole("combobox")).toBeFocused();
  await page.keyboard.type("logs-test");
  await expect(palette.getByText("Find (in the first 100 of each list)")).toBeVisible();
  await palette.getByRole("option", { name: /logs-test/ }).click();
  await expect(page).toHaveURL(/\/keys\?key=/);
  await expect(page.getByRole("dialog", { name: "logs-test" })).toBeVisible();

  await page.keyboard.press("Escape");
  await page.keyboard.press("ControlOrMeta+k");
  await expect(page.getByRole("dialog", { name: /command/i }).getByText("Recent")).toBeVisible();
  await expect(page.getByRole("dialog", { name: /command/i }).getByRole("option", { name: /logs-test/ })).toBeVisible();
});

test("the palette lists the organization's pages, and creating a project opens it on the Projects page", async ({ page }) => {
  await signInAsOwner(page);
  await expect(page).toHaveURL(/\/orgs\/[^/]+$/);
  const palette = page.getByRole("dialog", { name: /command/i });
  await page.keyboard.press("ControlOrMeta+k");
  await expect(palette.getByRole("combobox")).toBeFocused();
  await page.keyboard.type("Providers");
  await palette
    .getByRole("option", { name: /Providers/ })
    .first()
    .click();
  await expect(page).toHaveURL(/\/orgs\/[^/]+\/providers$/);
  await expect(page.getByRole("heading", { level: 1, name: "Providers" })).toBeVisible();

  await page.keyboard.press("ControlOrMeta+k");
  await expect(palette.getByRole("combobox")).toBeFocused();
  await page.keyboard.type("Create project");
  await palette.getByRole("option", { name: /Create project/ }).click();
  await expect(page).toHaveURL(/\/orgs\/[^/]+\/projects\?create=true$/);
  await expect(page.getByRole("dialog", { name: "Create project" })).toBeVisible();
});
