import { expect, test, enterEmail, expectSignedIn, signInAsOwner, stepTo, submitSteps, toastText } from "./support";
import { STACK_URLS, verifyDomain } from "./stack";

test.describe.configure({ mode: "serial" });

test("an identity provider is connected, and a new person signs in through it", async ({ page, browser }) => {
  await signInAsOwner(page);
  const org = /\/orgs\/([^/]+)/.exec(new URL(page.url()).pathname)?.[1] ?? "";
  await page.goto(`/settings/orgs/${org}/security`);
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
  await expect(page.getByRole("option", { name: "Add “example.com”" })).toBeVisible();
  await domains.press("Enter");
  await expect(dialog.getByText("example.com", { exact: true })).toBeVisible();
  await stepTo(dialog, dialog.getByRole("combobox", { name: "People without an invitation" }));
  await dialog.getByRole("combobox", { name: "People without an invitation" }).click();
  await page.getByRole("option", { name: /Join as Member/ }).click();
  await submitSteps(dialog, "Connect");
  await expect(toastText(page, "Identity provider connected")).toBeVisible();
  await expect(page.getByText("Register this redirect URL at the provider.")).toBeVisible();
  await expect(page.getByRole("switch", { name: "Require single sign-on" })).not.toHaveAttribute("aria-disabled", "true");
  verifyDomain("example.com");
  await page.reload();
  await expect(page.getByText("Verified; its addresses sign in through the provider.")).toBeVisible();

  const other = await browser.newContext();
  const visitor = await other.newPage();
  await visitor.goto("/sign-in");
  await enterEmail(visitor, "sso@example.com");
  await visitor.getByRole("button", { name: "Continue with SSO" }).click();
  await expect(visitor.getByRole("heading", { name: "Mock identity provider" })).toBeVisible();
  await visitor.getByLabel("Email").fill("sso@example.com");
  await visitor.getByLabel("Name").fill("Sam Rivera");
  await visitor.getByRole("button", { name: "Sign in" }).click();
  await expectSignedIn(visitor);
  await visitor.goto("/settings/account/sessions");
  await expect(visitor.getByText("SSO", { exact: true }).first()).toBeVisible();
  await other.close();
});
