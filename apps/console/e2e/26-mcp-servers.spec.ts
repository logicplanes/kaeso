import { expect, test, FIRST_PROJECT, signInAsOwner, toastText } from "./support";

test("an MCP server is added with its read-only tools, shown in its sheet and deleted", async ({ page }) => {
  await signInAsOwner(page);
  await page.getByRole("navigation", { name: "Console" }).getByRole("link", { name: "MCP servers" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "MCP servers" })).toBeVisible();
  await page
    .getByRole("main")
    .getByRole("button", { name: /^Add (a )?server$/ })
    .first()
    .click();

  const dialog = page.getByRole("dialog", { name: "Add MCP server" });
  await dialog.getByRole("textbox", { name: "Address" }).fill("http://mcp:8080/mcp");
  await dialog.getByRole("button", { name: "Check address" }).click();
  await expect(dialog.getByText("mock-x 2.0.0")).toBeVisible();
  await expect(dialog.getByText("3 tools")).toBeVisible();
  await dialog.getByRole("textbox", { name: "Handle" }).fill("e2e-x");
  await dialog.getByRole("button", { name: "Continue", exact: true }).click();

  await expect(dialog.getByRole("radio", { name: /No sign-in/ })).toBeChecked();
  await dialog.getByRole("button", { name: "Continue", exact: true }).click();

  const tools = dialog.getByRole("grid", { name: "Tools" });
  await expect(tools.getByRole("checkbox", { name: "Select search_posts" })).toBeChecked();
  await expect(tools.getByRole("checkbox", { name: "Select create_post" })).not.toBeChecked();
  await expect(tools.getByText("Destructive")).toBeVisible();
  await dialog.getByRole("button", { name: "Continue", exact: true }).click();

  await dialog.getByRole("checkbox", { name: FIRST_PROJECT }).click();
  await dialog.getByRole("button", { name: "Add server", exact: true }).click();
  await expect(toastText(page, "mock-x added")).toBeVisible();

  const sheet = page.getByRole("dialog", { name: "mock-x" });
  await expect(sheet.getByText("Working")).toBeVisible();
  await expect(sheet.getByText("e2e_x__search_posts")).toBeVisible();
  await expect(sheet.getByText(FIRST_PROJECT)).toBeVisible();

  await sheet.getByRole("button", { name: "Server actions" }).click();
  await page.getByRole("menuitem", { name: "Delete" }).click();
  await page.getByRole("alertdialog", { name: "Delete mock-x?" }).getByRole("button", { name: "Delete server" }).click();
  await expect(toastText(page, "mock-x deleted")).toBeVisible();
});
