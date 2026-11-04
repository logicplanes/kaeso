import type { Page } from "@playwright/test";
import { expect, test, signInToProject } from "./support";

test.describe.configure({ mode: "serial" });

const row = (page: Page, text: string, grid = "Models") =>
  page
    .getByRole("grid", { name: grid, exact: true })
    .getByRole("gridcell")
    .filter({ has: page.getByText(text, { exact: true }) });
const hit = (page: Page, model: string) =>
  page
    .getByRole("grid", { name: "Models in KPMR" })
    .getByRole("row")
    .filter({ has: page.getByText(model, { exact: true }) });

async function pick(page: Page, model: string) {
  await expect(async () => {
    await hit(page, model)
      .getByRole("checkbox", { name: `Select ${model}` })
      .check({ timeout: 2000 });
  }).toPass({ timeout: 10_000 });
}

async function openModels(page: Page) {
  await signInToProject(page);
  await page.getByRole("link", { name: "Models" }).click();
  await expect(page).toHaveTitle(/^Models · /);
}

test("an empty catalogue explains itself; models are added from KPMR in a batch", async ({ page }) => {
  await openModels(page);
  await expect(page.getByText("No models yet")).toBeVisible();
  await page.getByRole("button", { name: "Add models" }).first().click();
  const dialog = page.getByRole("dialog", { name: "Add models" });
  await dialog.getByRole("searchbox", { name: "Search models" }).fill("gpt-5.6");
  await pick(page, "gpt-5.6");
  await dialog.getByRole("searchbox", { name: "Search models" }).fill("gpt-5.4-mini");
  await pick(page, "gpt-5.4-mini");
  await expect(dialog.getByRole("button", { name: "Add 2 models" })).toBeEnabled();
  await expect(page.getByRole("region", { name: "Selection" })).toHaveCount(0);
  await dialog.getByRole("button", { name: "Add 2 models" }).click();
  await expect(page.getByText("Added 2 models")).toBeVisible();
  await expect(row(page, "gpt-5.6")).toBeVisible();
  await expect(row(page, "gpt-5.4-mini")).toBeVisible();
});

test("every current model of a provider is added at once, and models already there can't be chosen", async ({ page }) => {
  await openModels(page);
  await page.getByRole("button", { name: "Add models" }).first().click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("searchbox", { name: "Search models" }).fill("gpt-5.6");
  await expect(hit(page, "gpt-5.6").getByText("In catalogue")).toBeVisible();
  await expect(hit(page, "gpt-5.6").getByRole("checkbox", { name: "Select gpt-5.6" })).toHaveAttribute("aria-disabled", "true");
  await dialog.getByRole("button", { description: "Provider" }).click();
  await page.getByRole("option", { name: /^Anthropic/ }).click();
  await expect(dialog.getByRole("button", { description: "Provider" })).toHaveText(/Anthropic/);
  await dialog.getByRole("button", { name: "Add every current Anthropic model" }).click();
  await expect(dialog.getByText(/skips ones already here/)).toBeVisible();
  await dialog.getByRole("button", { name: "Add every current Anthropic model" }).click();
  await expect(page.getByText(/^Added \d+ models?$/)).toBeVisible();
  await expect(page.getByRole("grid", { name: "Models", exact: true }).getByRole("button", { name: /^Anthropic/ })).toBeVisible();
});

test("one model's values are reviewed before adding; a changed value becomes an override", async ({ page }) => {
  await openModels(page);
  await page.getByRole("button", { name: "Add models" }).first().click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("searchbox", { name: "Search models" }).fill("gpt-5.4-nano");
  await pick(page, "gpt-5.4-nano");
  await dialog.getByRole("button", { name: "Review values" }).click();
  await expect(dialog.getByRole("heading", { name: "Review gpt-5.4-nano" })).toBeVisible();
  await dialog.getByLabel("Input price").fill("0.25");
  await dialog.getByRole("button", { name: "Add model" }).click();
  await expect(page.getByText("Added gpt-5.4-nano")).toBeVisible();
  await expect(row(page, "gpt-5.4-nano").getByText("Overridden 1")).toBeVisible();
});

test("models are picked from the keyboard: Space selects, a row's review button opens its values, ⌘Enter adds", async ({ page }) => {
  await openModels(page);
  await page.getByRole("button", { name: "Add models" }).first().click();
  const dialog = page.getByRole("dialog", { name: "Add models" });
  const search = dialog.getByRole("searchbox", { name: "Search models" });
  await expect(search).toBeFocused();
  await page.keyboard.type("gpt-4.1");
  const hits = dialog.getByRole("grid", { name: "Models in KPMR" }).getByRole("rowgroup").last().getByRole("row");
  await expect(hits).toHaveCount(2);
  await expect(dialog.getByText("Requests to these models fail until someone adds a key for OpenAI on the Providers page.")).toBeVisible();
  // A Tab pressed as the dialog's resize animation ends can drop focus to the first field.
  await expect(dialog).not.toHaveAttribute("data-resizing");
  for (const control of [
    dialog.getByRole("button", { name: "Clear search" }),
    dialog.getByRole("button", { name: "Include deprecated" }),
    dialog.getByRole("link", { name: "Open providers" }),
  ]) {
    await page.keyboard.press("Tab");
    await expect(control).toBeFocused();
  }
  await page.keyboard.press("Tab");
  await expect(hit(page, "gpt-4.1")).toBeFocused();
  await page.keyboard.press("ArrowUp");
  await expect(dialog.getByRole("checkbox", { name: "Select all rows" })).toBeFocused();
  for (const column of ["Model", "Context", "Per 1M in / out", "Released"]) {
    await page.keyboard.press("ArrowRight");
    await expect(dialog.getByRole("columnheader", { name: column }).getByRole("button")).toBeFocused();
  }
  await page.keyboard.press("ArrowDown");
  await expect(hit(page, "gpt-4.1")).toBeFocused();
  await page.keyboard.press("Space");
  await expect(dialog.getByRole("button", { name: "Add 1 model" })).toBeVisible();
  await page.keyboard.press("ArrowDown");
  await expect(hit(page, "gpt-4.1-mini")).toBeFocused();
  await page.keyboard.press("Space");
  await expect(dialog.getByRole("button", { name: "Add 2 models" })).toBeVisible();
  await page.keyboard.press("Space");
  await expect(dialog.getByRole("button", { name: "Add 1 model" })).toBeVisible();
  await page.keyboard.press("ControlOrMeta+Enter");
  await expect(page.getByText("Added 1 model")).toBeVisible();
  await expect(row(page, "gpt-4.1")).toBeVisible();

  await page.getByRole("button", { name: "Add models" }).first().click();
  await expect(search).toBeFocused();
  await page.keyboard.press("ControlOrMeta+a");
  await page.keyboard.type("gpt-4.1-mini");
  await expect(hits).toHaveCount(1);
  const review = hit(page, "gpt-4.1-mini").getByRole("button", { name: "Review gpt-4.1-mini" });
  for (let i = 0; i < 16; i++) {
    const focused = await expect(hit(page, "gpt-4.1-mini"))
      .toBeFocused({ timeout: 200 })
      .then(
        () => true,
        () => false,
      );
    if (focused) break;
    await page.keyboard.press("Tab");
  }
  await page.keyboard.press("ArrowRight");
  await expect(review).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("dialog").getByRole("heading", { name: "Review gpt-4.1-mini" })).toBeVisible();
  await page.keyboard.press("ControlOrMeta+Enter");
  await expect(page.getByText("Added gpt-4.1-mini")).toBeVisible();
  await expect(row(page, "gpt-4.1-mini")).toBeVisible();
});

test("a model KPMR doesn't list is added as a custom model with its required fields", async ({ page }) => {
  await openModels(page);
  await page.getByRole("button", { name: "Add models" }).first().click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("button", { name: "Add a custom model" }).click();
  await dialog.getByLabel("Model id").fill("acme-internal-1");
  await dialog.getByLabel("Model id").press("Enter");
  await expect(dialog.getByText("KPMR doesn't list it")).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Add model" })).toBeDisabled();
  await dialog.getByRole("group", { name: "Inputs" }).getByRole("checkbox", { name: "Text" }).click();
  await dialog.getByRole("group", { name: "Outputs" }).getByRole("checkbox", { name: "Text" }).click();
  await dialog.getByLabel("Context").fill("128000");
  await dialog.getByLabel("Output limit").fill("8000");
  await dialog.getByLabel("Input price").fill("1");
  await dialog.getByLabel("Output price").fill("2");
  await dialog.getByRole("combobox", { name: "Sends as" }).click();
  await page.getByRole("option", { name: "OpenAI Chat Completions" }).click();
  await dialog.getByRole("button", { name: "Add model" }).click();
  await expect(page.getByText("Added acme-internal-1")).toBeVisible();
  await expect(row(page, "acme-internal-1").getByText("Custom")).toBeVisible();
});

test("the model sheet shows where values come from, resets an override and edits overrides", async ({ page }) => {
  await openModels(page);
  await row(page, "gpt-5.4-nano").click();
  const sheet = page.getByRole("dialog", { name: "GPT-5.4 nano" });
  await expect(sheet.getByText("override", { exact: true })).toHaveCount(1);
  await sheet.getByRole("button", { name: "Reset to KPMR" }).click();
  await expect(page.getByText("Input price reset to KPMR")).toBeVisible();
  await expect(sheet.getByText("override", { exact: true })).toHaveCount(0);

  await page.goto(page.url().replace(/\?.*$/, ""));
  await row(page, "gpt-5.6").click();
  const other = page.getByRole("dialog", { name: "GPT-5.6" });
  await other.getByRole("button", { name: "Edit overrides" }).click();
  await other.getByLabel("Timeout").fill("30");
  await other.getByRole("button", { name: "Save overrides" }).click();
  await expect(page.getByText("Overrides saved")).toBeVisible();
  await expect(other.getByText("30 s")).toBeVisible();
});

test("aliases: build a route with a fallback, warn about a shadowed model, rename, delete", async ({ page }) => {
  await openModels(page);
  await page.getByRole("tab", { name: /Aliases/ }).click();
  await expect(page.getByText("No aliases yet")).toBeVisible();
  await page.getByRole("button", { name: "Create alias" }).first().click();
  await expect(page.getByRole("heading", { name: "New alias" })).toBeVisible();
  await page.getByLabel("Name").fill("fast");
  const steps = page.getByRole("tree", { name: "Route of fast" });
  await steps.getByRole("treeitem", { name: /^Model.*No model yet/ }).click();
  const panel = page.getByRole("complementary", { name: "Step settings" });
  await panel.getByRole("button", { name: /^Model/ }).click();
  await page.getByRole("option", { name: /^GPT-5\.4 mini/ }).click();
  await steps.getByRole("treeitem", { name: /^Add a fallback to/ }).click();
  await page.getByRole("menuitem", { name: "Fall back to a model" }).click();
  await panel.getByRole("button", { name: /^Model/ }).click();
  await page.getByRole("option", { name: /^GPT-5\.6/ }).click();
  await page.getByRole("button", { name: "Create alias" }).click();
  await expect(page.getByText("Alias fast created")).toBeVisible();
  await expect(page.getByRole("heading", { name: "fast" })).toBeVisible();
  await page.getByRole("tab", { name: "Test" }).click();
  await expect(panel.getByRole("heading", { name: "Test a request" })).toBeVisible();
  await expect(steps.getByRole("treeitem", { name: /Try 2/ })).toBeVisible();
  await panel.getByRole("button", { name: "Send" }).click();
  await expect(panel.getByText("Steps it took")).toBeVisible();
  await expect(panel.getByRole("listitem").filter({ hasText: "GPT-5.4 mini" })).toContainText("fell back");
  await expect(panel.getByRole("listitem").filter({ hasText: "GPT-5.6" })).toBeVisible();
  await expect(panel.getByRole("button", { name: "Open in request log" })).toBeVisible();
  await panel.getByRole("radio", { name: "Preview" }).click();
  await expect(panel.getByText("Try 1; tried first")).toBeVisible();
  await expect(panel.getByText("Try 2; tried when try 1 fails")).toBeVisible();
  await page.getByRole("tab", { name: "Editor" }).click();

  await page.getByRole("button", { name: "Back to aliases" }).click();
  await expect(row(page, "fast", "Aliases")).toContainText("GPT-5.4 mini → GPT-5.6");
  await page.getByRole("button", { name: "Create alias" }).first().click();
  await page.getByLabel("Name").fill("gpt-5.4-nano");
  await expect(page.getByText("Apps that send `gpt-5.4-nano` get this alias, not the model with the same name.")).toBeVisible();
  await page.getByRole("treeitem", { name: /^Model.*No model yet/ }).click();
  await page.getByRole("complementary", { name: "Step settings" }).getByRole("button", { name: /^Model/ }).click();
  await page.getByRole("option", { name: /^GPT-5\.6/ }).click();
  await page.getByRole("button", { name: "Create alias" }).click();
  await expect(page.getByText("Alias gpt-5.4-nano created")).toBeVisible();
  await page.getByRole("button", { name: "Back to aliases" }).click();
  await expect(row(page, "gpt-5.4-nano", "Aliases").getByText("Shadows a model")).toBeVisible();

  await row(page, "fast", "Aliases").click();
  await page.getByRole("treeitem", { name: /^Start/ }).click();
  await page.getByLabel("Name").fill("quick");
  await page.getByRole("button", { name: "Save" }).click();
  await expect(page.getByText("Alias quick saved")).toBeVisible();
  await page.getByRole("tab", { name: "Audit log" }).click();
  const audit = page.getByRole("grid", { name: "Audit log" });
  await expect(audit.getByText("Alias changed")).toBeVisible();
  await audit.getByText("Alias created").click();
  await page.getByRole("dialog", { name: /Alias created/ }).getByRole("button", { name: "Restore this version" }).click();
  await expect(page.getByText("Version restored; save to make it live")).toBeVisible();
  await expect(page.getByRole("tab", { name: "Editor" })).toHaveAttribute("aria-selected", "true");
  await expect(page.getByLabel("Name")).toHaveValue("fast");
  await page.getByRole("button", { name: "Delete alias" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Delete alias" }).click();
  await expect(page.getByText("Alias quick deleted")).toBeVisible();
  await expect(row(page, "quick", "Aliases")).toHaveCount(0);
});

test("a model is removed from the catalogue after saying what happens", async ({ page }) => {
  await openModels(page);
  await row(page, "acme-internal-1").click();
  const sheet = page.getByRole("dialog", { name: "acme-internal-1" });
  await sheet.getByRole("button", { name: "Model actions" }).click();
  await page.getByRole("menuitem", { name: "Remove from catalogue" }).click();
  const confirm = page.getByRole("alertdialog");
  await expect(confirm.getByText("Requests naming it are refused")).toBeVisible();
  await confirm.getByRole("button", { name: "Remove from catalogue" }).click();
  await expect(page.getByText("Model removed from the catalogue")).toBeVisible();
  await expect(row(page, "acme-internal-1")).toHaveCount(0);
});
