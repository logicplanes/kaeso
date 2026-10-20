import { expect, test, NEW_PASSWORD, OWNER, PASSWORD, clearMail, enterEmail, expectSignedIn, mailLink, signInWithPassword, toastText } from "./support";
import { setupLinkToken } from "./stack";

test.describe.configure({ mode: "serial" });

test("setup creates the first user and organization", async ({ page }) => {
  await clearMail();
  await page.goto(`/setup#token=${setupLinkToken()}`);
  await expect(page).toHaveURL(/\/setup$/);
  await expect(page.getByRole("heading", { name: "Set up Kaeso" })).toBeVisible();
  await page.getByLabel("Name", { exact: true }).fill(OWNER.name);
  await page.getByLabel("Email").fill(OWNER.email);
  await page.getByLabel("Password").fill("short");
  await expect(page.getByText("7 more characters to go.")).toBeVisible();
  await page.getByLabel("Password").fill(PASSWORD);
  await page.getByLabel("Organization name").fill(OWNER.org);
  await page.getByRole("button", { name: "Set up Kaeso" }).click();
  await expectSignedIn(page);
  await expect(page).toHaveTitle(/Overview · Default · Kaeso|Overview · .* · Kaeso/);
  await expect(page.getByRole("button", { name: `${OWNER.org}: switch or open settings` })).toBeVisible();
});

test("a used setup link explains how to get a new one", async ({ page }) => {
  await page.goto(`/setup#token=${setupLinkToken()}`);
  await page.getByLabel("Name", { exact: true }).fill("Someone");
  await page.getByLabel("Email").fill("someone@example.com");
  await page.getByLabel("Password").fill(PASSWORD);
  await page.getByLabel("Organization name").fill("Other");
  await page.getByRole("button", { name: "Set up Kaeso" }).click();
  await expect(page.getByText("kaeso setup-link")).toBeVisible();
});

test("sign out, a wrong password, then sign in", async ({ page }) => {
  await signInWithPassword(page, OWNER.email, PASSWORD);
  await expectSignedIn(page);
  await page.getByRole("button", { name: `${OWNER.org}: switch or open settings` }).click();
  await page.getByRole("menuitem", { name: "Sign out" }).click();
  await expect(page).toHaveURL(/\/sign-in/);
  await enterEmail(page, OWNER.email);
  await page.getByLabel("Password", { exact: true }).fill("not the password at all");
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await expect(toastText(page, "The email or password is wrong.")).toBeVisible();
  await page.getByLabel("Password", { exact: true }).fill(PASSWORD);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await expectSignedIn(page);
});

test("a protected page sends you to sign in and back", async ({ page }) => {
  await page.goto("/orgs/org_doesnotexist/projects/proj_doesnotexist");
  await expect(page).toHaveURL(/\/sign-in\?next=/);
  await expect(page.getByRole("heading", { name: "Sign in", exact: true })).toBeVisible();
  await expect(page.getByLabel("Password", { exact: true })).toHaveCount(0);
  await expect(page.getByRole("link", { name: "Create an account" })).toHaveCount(0);
});

test("a sign-in link from the email signs you in", async ({ page }) => {
  await clearMail();
  await page.goto("/sign-in");
  await enterEmail(page, OWNER.email);
  await page.getByRole("button", { name: "Email me a sign-in link" }).click();
  await expect(page.getByRole("heading", { name: "Check your email" })).toBeVisible();
  const link = await mailLink(OWNER.email, /\/auth\/magic-link#token=/);
  await page.goto(new URL(link).pathname + new URL(link).hash);
  await expect(page.getByRole("heading", { name: `Sign in as ${OWNER.email}` })).toBeVisible();
  await expect(page).not.toHaveURL(/token=/);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await expectSignedIn(page);
});

test("a password reset ends with a new password", async ({ page }) => {
  await clearMail();
  await page.goto("/sign-in");
  await enterEmail(page, OWNER.email);
  await page.getByRole("link", { name: "Forgot your password?" }).click();
  await expect(page.getByRole("heading", { name: "Reset your password" })).toBeVisible();
  await page.getByLabel("Email").fill(OWNER.email);
  await page.getByRole("button", { name: "Email me a link" }).click();
  await expect(page.getByRole("heading", { name: "Check your email" })).toBeVisible();
  const link = await mailLink(OWNER.email, /\/auth\/password-reset#token=/);
  await page.goto("about:blank");
  await page.goto(new URL(link).pathname + new URL(link).hash);
  await page.getByLabel("New password").fill(NEW_PASSWORD);
  await page.getByRole("button", { name: "Set password" }).click();
  await expect(page.getByText("Password changed; sign in with the new one")).toBeVisible();
  await enterEmail(page, OWNER.email);
  await page.getByLabel("Password", { exact: true }).fill(NEW_PASSWORD);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await expectSignedIn(page);
});

test("an unknown page inside the console says so", async ({ page }) => {
  await signInWithPassword(page, OWNER.email, NEW_PASSWORD);
  await expectSignedIn(page);
  await page.goto("/no/such/page");
  await expect(page.getByText("This page doesn't exist")).toBeVisible();
});
