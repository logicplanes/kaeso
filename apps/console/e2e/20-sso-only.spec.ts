import type { Page } from "@playwright/test";
import { expect, test, expectSignedIn, stepTo, submitSteps, toastText } from "./support";
import { SSO_CONSOLE_URL, STACK_URLS, operatorSignInToken, setupLinkToken } from "./stack";

test.describe.configure({ mode: "serial" });
test.use({ baseURL: SSO_CONSOLE_URL });

const ADMIN = { name: "Ines Moreau", email: "ines@example.com", org: "Northwind SSO" };

async function signInAtProvider(page: Page, email: string, name: string): Promise<void> {
  await expect(page.getByRole("heading", { name: "Mock identity provider" })).toBeVisible();
  await page.getByLabel("Email").fill(email);
  await page.getByLabel("Name").fill(name);
  await page.getByRole("button", { name: "Sign in" }).click();
}

test("the first admin sets up an install without passwords and connects the identity provider", async ({ page }) => {
  await page.goto(`/setup#token=${setupLinkToken("gateway-sso")}`);
  await expect(page.getByText("Create the first user, an instance admin, and the organization they own; passwords are off here.")).toBeVisible();
  await expect(page.getByLabel("Password")).toHaveCount(0);
  await expect(page.getByText("kaeso user sign-in-link")).toBeVisible();
  await page.getByLabel("Name", { exact: true }).fill(ADMIN.name);
  await page.getByLabel("Email").fill(ADMIN.email);
  await page.getByLabel("Organization name").fill(ADMIN.org);
  await page.getByRole("button", { name: "Set up Kaeso" }).click();
  await expect(page).toHaveURL(/\/settings\/orgs\/[^/]+\/security$/);

  await page.getByRole("button", { name: "Connect an identity provider" }).click();
  const dialog = page.getByRole("dialog", { name: "Connect an identity provider" });
  await dialog.getByLabel("Issuer URL").fill(STACK_URLS.idp);
  await stepTo(dialog, dialog.getByLabel("Client id"));
  await dialog.getByLabel("Client id").fill("kaeso-console");
  await dialog.getByLabel("Client secret").fill("idp-client-secret");
  const domains = dialog.getByRole("combobox", { name: "Email domains" });
  await stepTo(dialog, domains);
  await domains.click();
  await domains.fill("example.com");
  await domains.press("Enter");
  await stepTo(dialog, dialog.getByRole("combobox", { name: "People without an invitation" }));
  await dialog.getByRole("combobox", { name: "People without an invitation" }).click();
  await page.getByRole("option", { name: /Join as Member/ }).click();
  await stepTo(dialog, dialog.getByLabel("Name on the button"));
  await dialog.getByLabel("Name on the button").fill("Northwind ID");
  await dialog.getByRole("switch", { name: "Start without a click" }).click();
  await submitSteps(dialog, "Connect");
  await expect(toastText(page, "Identity provider connected")).toBeVisible();
  await expect(page.getByText("Says Continue with Northwind ID and starts on its own.")).toBeVisible();
  await expect(page.getByText(new RegExp(`${SSO_CONSOLE_URL}/sign-in\\?org=org_`))).toBeVisible();
});

test("a person goes straight to the identity provider and comes back to the page they asked for", async ({ browser }) => {
  const context = await browser.newContext({ baseURL: SSO_CONSOLE_URL });
  const page = await context.newPage();
  await page.goto("/settings/account/sessions");
  await signInAtProvider(page, "sam@example.com", "Sam Rivera");
  await expect(page).toHaveURL(/\/settings\/account\/sessions$/);
  await expect(page.getByRole("heading", { name: "Sessions", level: 1 })).toBeVisible();

  await page.goto("/");
  await expectSignedIn(page);
  await page.getByRole("button", { name: `${ADMIN.org}: switch or open settings` }).click();
  await page.getByRole("menuitem", { name: "Sign out" }).click();
  await expect(page).toHaveURL(/\/sign-in$/);
  await expect(toastText(page, "You're signed out")).toBeVisible();
  await expect(page.getByRole("button", { name: "Continue with Northwind ID" })).toBeVisible();
  await expect(page.getByRole("textbox", { name: "Email" })).toHaveCount(0);
  await page.getByRole("button", { name: "Continue with Northwind ID" }).click();
  await signInAtProvider(page, "sam@example.com", "Sam Rivera");
  await expectSignedIn(page);
  await context.close();
});

test("a start link sends a bookmark straight to the identity provider", async ({ browser }) => {
  const context = await browser.newContext({ baseURL: SSO_CONSOLE_URL });
  const page = await context.newPage();
  await page.goto("/platform/v1/auth/oidc/start?next=%2Fsettings%2Faccount%2Fsessions");
  await signInAtProvider(page, "sam@example.com", "Sam Rivera");
  await expect(page).toHaveURL(/\/settings\/account\/sessions$/);
  await expect(page.getByRole("heading", { name: "Sessions", level: 1 })).toBeVisible();
  await context.close();
});

test("a refused sign-in comes back to the sign-in page and waits for a click", async ({ browser }) => {
  const context = await browser.newContext({ baseURL: SSO_CONSOLE_URL });
  const page = await context.newPage();
  await page.goto("/sign-in?next=/settings/account");
  await signInAtProvider(page, "eve@elsewhere.example", "Eve");
  await expect(page).toHaveURL(/\/sign-in\?next=%2Fsettings%2Faccount&org=org_[a-z0-9]+$/);
  await expect(toastText(page, "This organization's SSO only signs in addresses of example.com.")).toBeVisible();
  await expect(page.getByRole("button", { name: "Continue with Northwind ID" })).toBeVisible();
  await context.close();
});

test("the operator's sign-in link lets the first admin in without a password", async ({ browser }) => {
  const context = await browser.newContext({ baseURL: SSO_CONSOLE_URL });
  const page = await context.newPage();
  await page.goto(`/auth/magic-link#token=${operatorSignInToken(ADMIN.email)}`);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await expectSignedIn(page);
  await expect(page.getByRole("button", { name: `${ADMIN.org}: switch or open settings` })).toBeVisible();
  await context.close();
});
