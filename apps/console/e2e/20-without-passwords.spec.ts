import type { Page } from "@playwright/test";
import { expect, test, enterEmail, expectSignedIn, stepTo, submitSteps, toastText } from "./support";
import { SSO_CONSOLE_URL, STACK_URLS, operatorSignInToken, setupLinkToken, verifyDomain } from "./stack";

test.describe.configure({ mode: "serial" });
test.use({ baseURL: SSO_CONSOLE_URL });

const ADMIN = { name: "Ines Moreau", email: "ines@example.com", org: "Northwind SSO" };
let org = "";

async function signInAtProvider(page: Page, email: string, name: string): Promise<void> {
  await expect(page.getByRole("heading", { name: "Mock identity provider" })).toBeVisible();
  await page.getByLabel("Email").fill(email);
  await page.getByLabel("Name").fill(name);
  await page.getByRole("button", { name: "Sign in" }).click();
}

test("the first admin sets up an install without passwords and connects the identity provider", async ({ page }) => {
  await page.goto(`/setup#token=${setupLinkToken("gateway-sso")}`);
  await expect(page.getByText("Create the first user, an instance admin, and the organization they own; passwords are off here.")).toBeVisible();
  await expect(page.getByLabel("Password", { exact: true })).toHaveCount(0);
  await expect(page.getByText("kaeso user sign-in-link")).toBeVisible();
  await page.getByLabel("Name", { exact: true }).fill(ADMIN.name);
  await page.getByLabel("Email").fill(ADMIN.email);
  await page.getByLabel("Organization name").fill(ADMIN.org);
  await page.getByRole("button", { name: "Set up Kaeso" }).click();
  await expect(page).toHaveURL(/\/settings\/orgs\/[^/]+\/security$/);
  org = /\/settings\/orgs\/([^/]+)\/security$/.exec(new URL(page.url()).pathname)?.[1] ?? "";

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
  verifyDomain("example.com", "gateway-sso");
  await page.reload();
  await expect(page.getByText("Its addresses sign in through the provider.")).toBeVisible();
  await expect(page.getByText("Says Continue with Northwind ID and starts on its own.")).toBeVisible();
  await expect(page.getByText(`${SSO_CONSOLE_URL}/platform/v1/auth/oidc/start?org=${org}`)).toHaveCount(0);
});

test("a person enters their email, goes straight to the identity provider and comes back to the page they asked for", async ({ browser }) => {
  const context = await browser.newContext({ baseURL: SSO_CONSOLE_URL });
  const page = await context.newPage();
  await page.goto("/settings/account/sessions");
  await expect(page).toHaveURL(/\/sign-in\?next=%2Fsettings%2Faccount%2Fsessions$/);
  await expect(page.getByRole("button", { name: "Continue with Northwind ID" })).toHaveCount(0);
  await enterEmail(page, "sam@example.com");
  await signInAtProvider(page, "sam@example.com", "Sam Rivera");
  await expect(page).toHaveURL(/\/settings\/account\/sessions$/);
  await expect(page.getByRole("heading", { name: "Sessions", level: 1 })).toBeVisible();

  await page.goto("/");
  await expectSignedIn(page);
  await page.getByRole("button", { name: `${ADMIN.org}: switch or open settings` }).click();
  await page.getByRole("menuitem", { name: "Sign out" }).click();
  await expect(page).toHaveURL(/\/sign-in$/);
  await expect(toastText(page, "You're signed out")).toBeVisible();
  await expect(page.getByRole("textbox", { name: "Email" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Continue with Northwind ID" })).toHaveCount(0);
  await enterEmail(page, "sam@example.com");
  await signInAtProvider(page, "sam@example.com", "Sam Rivera");
  await expectSignedIn(page);
  await context.close();
});

test("a link that names the organization still asks for the email before the identity provider", async ({ browser }) => {
  const context = await browser.newContext({ baseURL: SSO_CONSOLE_URL });
  const page = await context.newPage();
  const link = await page.goto(`/platform/v1/auth/oidc/start?org=${org}&next=%2Fsettings%2Faccount%2Fsessions`);
  expect(link?.status()).toBe(405);
  await page.goto(`/sign-in?next=%2Fsettings%2Faccount%2Fsessions&org=${org}&start=sso`);
  await expect(page.getByRole("textbox", { name: "Email" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Mock identity provider" })).toHaveCount(0);
  await enterEmail(page, "sam@example.com");
  await signInAtProvider(page, "sam@example.com", "Sam Rivera");
  await expect(page).toHaveURL(/\/settings\/account\/sessions$/);

  await page.goto(`/sign-in?next=%2Fsettings%2Faccount%2Fsessions&org=${org}&start=sso`);
  await expect(page).toHaveURL(/\/settings\/account\/sessions$/);
  await expect(page.getByRole("heading", { name: "Sessions", level: 1 })).toBeVisible();
  await context.close();
});

test("a refused sign-in comes back to the sign-in page and asks for the email again", async ({ browser }) => {
  const context = await browser.newContext({ baseURL: SSO_CONSOLE_URL });
  const page = await context.newPage();
  await page.goto("/sign-in?next=/settings/account");
  await enterEmail(page, "sam@example.com");
  await signInAtProvider(page, "eve@elsewhere.example", "Eve");
  await expect(page).toHaveURL(/\/sign-in\?next=%2Fsettings%2Faccount$/);
  await expect(toastText(page, "This organization's single sign-on doesn't sign in addresses at your domain.")).toBeVisible();
  await expect(page.getByRole("textbox", { name: "Email" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Mock identity provider" })).toHaveCount(0);
  await context.close();
});

test("an address that no organization's provider signs in has no way in", async ({ browser }) => {
  const context = await browser.newContext({ baseURL: SSO_CONSOLE_URL });
  const page = await context.newPage();
  await page.goto("/sign-in");
  await enterEmail(page, "eve@elsewhere.example");
  await expect(page.getByText("There's no way to sign in with eve@elsewhere.example on this install. Ask an administrator.")).toBeVisible();
  await expect(page.getByRole("button", { name: "Continue with Northwind ID" })).toHaveCount(0);
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
