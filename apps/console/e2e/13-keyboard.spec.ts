import { expect, test, NEW_PASSWORD, OWNER, enterEmail, openDefaultProject } from "./support";

test.describe.configure({ mode: "serial" });

test("the main flow works from the keyboard alone", async ({ page }) => {
  await page.goto("/sign-in");
  await expect(page.getByLabel("Email")).toBeFocused();
  await page.keyboard.type(OWNER.email);
  await page.keyboard.press("Enter");
  await expect(page.getByLabel("Password", { exact: true })).toBeFocused();
  await page.keyboard.type(NEW_PASSWORD);
  await page.keyboard.press("Enter");
  await expect(page.getByRole("navigation", { name: "Console" })).toBeVisible();
  const memberships = (await (await page.request.get("/platform/v1/me/memberships")).json()) as {
    data: { org_id: string; projects: { id: string; is_default: boolean }[] }[];
  };
  const home = memberships.data[0];
  const project = home?.projects.find((p) => p.is_default)?.id;
  await page.goto(`/orgs/${home?.org_id}/projects/${project}`);
  await expect(page.getByRole("navigation", { name: "Console" })).toBeVisible();
  await page.keyboard.press("Tab");
  await expect(page.getByRole("link", { name: "Skip to content" })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("main")).toBeFocused();

  await page.keyboard.press("g");
  await page.keyboard.press("k");
  await expect(page).toHaveTitle(/^API keys · /);
  await page.keyboard.press("c");
  const dialog = page.getByRole("dialog", { name: "Create key" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByLabel("Name")).toBeFocused();
  await page.keyboard.type("keyboard-only");
  await page.keyboard.press("ControlOrMeta+Enter");
  await expect(dialog.getByText("Step 2 of 4", { exact: true })).toBeVisible();
  await expect(dialog.getByRole("combobox", { name: "Expires" })).toBeFocused();
  await page.keyboard.press("ControlOrMeta+Enter");
  await expect(dialog.getByText("Step 3 of 4", { exact: true })).toBeVisible();
  await expect(dialog.getByRole("radio", { name: "Inference", exact: true })).toBeFocused();
  await page.keyboard.press("ControlOrMeta+Enter");
  await expect(dialog.getByText("Step 4 of 4", { exact: true })).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Models" })).toBeFocused();
  await page.keyboard.press("ControlOrMeta+Enter");
  const reveal = page.getByRole("dialog", { name: "Key created" });
  await expect(reveal).toBeVisible();
  const secret = (await reveal.getByRole("textbox", { name: "Key for keyboard-only" }).textContent()) ?? "";
  const sent = await page.request.post("/v1/chat/completions", {
    headers: { authorization: `Bearer ${secret}` },
    data: { model: "gpt-5.4-mini", messages: [{ role: "user", content: "From the keyboard pass" }] },
  });
  expect(sent.ok()).toBe(true);
  await reveal.getByRole("button", { name: "Copy key" }).focus();
  await page.keyboard.press("Enter");
  await expect(reveal.getByRole("button", { name: "Key copied" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(reveal).toBeHidden();

  await page.keyboard.press("ControlOrMeta+k");
  await expect(page.getByRole("dialog", { name: "Command palette" }).getByRole("combobox")).toBeFocused();
  await page.keyboard.type("Request logs");
  await page.keyboard.press("Enter");
  await expect(page).toHaveTitle(/^Request logs · /);
  const firstRow = page.getByRole("table", { name: "Requests" }).getByRole("row").nth(1);
  await expect(async () => {
    await page.reload();
    await expect(firstRow.locator("time")).toBeVisible({ timeout: 2000 });
  }).toPass({ timeout: 30_000 });
  await firstRow.focus();
  await page.keyboard.press("Enter");
  const inspector = page.getByRole("dialog").filter({ has: page.getByRole("tab", { name: "Overview" }) });
  await expect(inspector).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(inspector).toBeHidden();

  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.keyboard.press("?");
  const sheet = page.getByRole("dialog", { name: "Keyboard shortcuts" });
  await expect(sheet).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(sheet).toBeHidden();
});

test("the shell, a list, a dialog and the inspector expose names and roles", async ({ page }) => {
  await page.goto("/sign-in");
  await enterEmail(page, OWNER.email);
  await page.getByLabel("Password", { exact: true }).fill(NEW_PASSWORD);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await expect(page.getByRole("navigation", { name: "Console" })).toBeVisible();
  await expect(page.getByRole("main")).toBeVisible();
  await expect(page.getByRole("heading", { level: 1 })).toHaveCount(1);

  await openDefaultProject(page);
  await page.getByRole("link", { name: "API keys" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "API keys" })).toBeVisible();
  await expect(page.getByRole("grid", { name: /keys/i }).first()).toBeVisible();

  await page.getByRole("button", { name: "Create key" }).first().click();
  const dialog = page.getByRole("dialog", { name: "Create key" });
  await expect(dialog.getByRole("textbox", { name: "Name" })).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Cancel" })).toBeVisible();
  await page.keyboard.press("Escape");

  await page.getByRole("link", { name: "Request logs" }).click();
  await page.getByRole("table", { name: "Requests" }).getByRole("row").nth(1).click();
  const inspector = page.getByRole("dialog").filter({ has: page.getByRole("tab", { name: "Overview" }) });
  await expect(inspector.getByRole("tablist")).toBeVisible();
  for (const tab of ["Overview", "Request", "Reply", "Raw"]) await expect(inspector.getByRole("tab", { name: tab })).toBeVisible();
});
