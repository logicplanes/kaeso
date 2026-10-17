import { createHmac } from "node:crypto";
import { resetUserTwoFactor } from "./stack";
import { expect, test, NEW_PASSWORD, OWNER, expectSignedIn, signInAsOwner, signInWithPassword } from "./support";

test.describe.configure({ mode: "serial" });

let ownerTwoFactorMayBeOn = false;

test.afterEach(() => {
  if (ownerTwoFactorMayBeOn) resetUserTwoFactor(OWNER.email);
  ownerTwoFactorMayBeOn = false;
});

function totp(secret: string, at = Date.now()): string {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
  let bits = "";
  for (const c of secret.replace(/=+$/, "").toUpperCase()) bits += alphabet.indexOf(c).toString(2).padStart(5, "0");
  const key = Buffer.from(bits.match(/.{8}/g)?.map((b) => parseInt(b, 2)) ?? []);
  const counter = Buffer.alloc(8);
  counter.writeBigUInt64BE(BigInt(Math.floor(at / 30_000)));
  const hmac = createHmac("sha1", key).update(counter).digest();
  const offset = (hmac[hmac.length - 1] ?? 0) & 0xf;
  const code = ((hmac.readUInt32BE(offset) & 0x7fffffff) % 1_000_000).toString();
  return code.padStart(6, "0");
}

test("profile, notifications, sessions and memberships", async ({ page }) => {
  await signInAsOwner(page);
  await page.goto("/settings");
  await expect(page).toHaveURL(/\/settings\/account\/profile$/);
  const name = page.getByRole("textbox", { name: "Name" });
  await name.fill("Ada O. Okafor");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByText("Name saved")).toBeVisible();
  await name.fill(OWNER.name);
  await page.getByRole("button", { name: "Save", exact: true }).click();

  await page.getByRole("link", { name: "Notifications" }).click();
  const budget = page.getByRole("switch", { name: "Budget alerts" });
  const before = await budget.isChecked();
  await budget.click();
  await expect(page.getByText("Notifications saved")).toBeVisible();
  await expect(budget).toBeChecked({ checked: !before });

  await page.getByRole("link", { name: "Sessions" }).click();
  await expect(page.getByText("This device")).toBeVisible();

  await page.getByRole("link", { name: "Memberships" }).click();
  await expect(page.getByRole("heading", { name: OWNER.org })).toBeVisible();
  await expect(page.getByText("Owner of the organization")).toBeVisible();
});

test("two-factor authentication: set up with the QR code's key, sign in with a code and a recovery code, then off", async ({ page, browser }) => {
  await signInAsOwner(page);
  await page.goto("/settings/account/security");
  await page.getByRole("button", { name: "Set up" }).click();
  const dialog = page.getByRole("dialog", { name: "Set up two-factor authentication" });
  await expect(dialog.getByRole("img", { name: "QR code to add Kaeso to an authenticator app" })).toBeVisible();
  const secret = (await dialog.locator("code").first().textContent()) ?? "";
  expect(secret).toMatch(/^[A-Z2-7]+=*$/);
  ownerTwoFactorMayBeOn = true;
  await dialog.getByRole("textbox", { name: "Code from the app" }).fill(totp(secret));

  const codes = page.getByRole("dialog", { name: "Store your recovery codes" });
  await expect(codes).toBeVisible();
  await expect(codes.getByRole("button", { name: "Done" })).toBeDisabled();
  const recovery = (await codes.locator("pre").first().innerText()).trim().split(/\s+/)[0] ?? "";
  expect(recovery).toMatch(/^[a-z0-9]{4,}(-[a-z0-9]{4,})*$/i);
  await codes.getByRole("checkbox", { name: "I stored these codes" }).click();
  await codes.getByRole("button", { name: "Done" }).click();
  await expect(page.getByText("10 of 10 left", { exact: false })).toBeVisible();

  const other = await browser.newContext();
  const elsewhere = await other.newPage();
  await signInWithPassword(elsewhere, OWNER.email, NEW_PASSWORD);
  await expect(elsewhere.getByText("Enter the 6-digit code from your authenticator app.")).toBeVisible();
  await elsewhere
    .getByRole("textbox")
    .first()
    .pressSequentially(totp(secret, Date.now() + 30_000));
  await expectSignedIn(elsewhere);
  await other.close();

  const third = await browser.newContext();
  const again = await third.newPage();
  await signInWithPassword(again, OWNER.email, NEW_PASSWORD);
  await again.getByRole("link", { name: "Use a recovery code" }).click();
  await again.getByLabel("Recovery code").fill(recovery);
  await again.getByRole("button", { name: "Verify" }).click();
  await expectSignedIn(again);
  await third.close();
  await page.reload();
  await expect(page.getByText("9 of 10 left", { exact: false })).toBeVisible();

  await page.getByRole("button", { name: "Turn off" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Turn off" }).click();
  await expect(page.getByText("Two-factor authentication turned off")).toBeVisible();
  ownerTwoFactorMayBeOn = false;
  await expect(page.getByRole("button", { name: "Set up" })).toBeVisible();
});
