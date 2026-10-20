import type { Page } from "@playwright/test";
import { expect, test, NEW_PASSWORD, clearMail, expectSignedIn, mailLink, signInAsOwner, toastText } from "./support";

test.describe.configure({ mode: "serial" });

const orgOf = (page: Page) => /\/orgs\/([^/]+)/.exec(new URL(page.url()).pathname)?.[1] ?? "";
const VIEWER = { name: "Sam Reed", email: "viewer@example.com" };
const MODELS = ["gpt-5.4-mini", "gpt-5.4-nano"];

let org = "";
let project = "";

async function setUp(page: Page) {
  const headers = { origin: new URL(page.url()).origin };
  org = orgOf(page);
  const api = `/platform/v1/orgs/${org}`;
  const credentials = (await (await page.request.get(`${api}/credentials`)).json()) as { data: { provider: string }[] };
  if (!credentials.data.some((c) => c.provider === "openai")) {
    const res = await page.request.post(`${api}/credentials`, {
      headers,
      data: { provider: "openai", name: "Mock", secret: "sk-proj-mock-0000000000000000openai" },
    });
    expect(res.ok()).toBe(true);
  }
  project = ((await (await page.request.post(`${api}/projects`, { headers, data: { name: "Comparisons" } })).json()) as { id: string }).id;
  const added = await page.request.post(`${api}/projects/${project}/models/batch`, { headers, data: { provider: "openai", model_ids: MODELS } });
  expect(added.ok()).toBe(true);
}

const column = (page: Page, name: RegExp) => page.getByRole("region", { name });
const reply = (page: Page, name: RegExp) => column(page, name).getByRole("article").last();
const lastPrompt = (page: Page, name: RegExp) => column(page, name).getByRole("group", { name: "Prompt" }).last();

test("one prompt streams from two models side by side, with metrics; Stop ends both", async ({ page }) => {
  await signInAsOwner(page);
  await setUp(page);
  await page.goto(`/orgs/${org}/projects/${project}`);
  await page.getByRole("navigation", { name: "Console" }).getByRole("link", { name: "Playground" }).click();
  await expect(page).toHaveTitle(/^Playground · /);

  await expect(page.getByRole("region", { name: /^GPT-5\.4 (mini|nano)$/ })).toHaveCount(1);
  await page.getByRole("button", { name: "Add column" }).click();
  await expect(page).toHaveURL(/models=openai%2Fgpt-5\.4-mini%2Copenai%2Fgpt-5\.4-nano|models=openai\/gpt-5\.4-mini,openai\/gpt-5\.4-nano/);
  const mini = /^GPT-5\.4 mini$/;
  const nano = /^GPT-5\.4 nano$/;

  const prompt = page.getByRole("textbox", { name: "Prompt" });
  await prompt.fill("Say hello [mock:gen=24] [mock:tps=400]");
  await page.getByRole("button", { name: "Send", exact: true }).click();
  for (const name of [mini, nano]) {
    await expect(lastPrompt(page, name)).toHaveText("Say hello [mock:gen=24] [mock:tps=400]");
    await expect(reply(page, name)).not.toBeEmpty();
    const metrics = column(page, name).getByRole("definition");
    await expect(metrics.filter({ hasText: /^\$/ })).toBeVisible({ timeout: 20_000 });
    await expect(metrics.filter({ hasText: /^\d[\d,]* in · \d[\d,]* out$/ })).toBeVisible();
    await expect(metrics.filter({ hasText: /^\d[\d.,]* (ms|s)$/ }).first()).toBeVisible();
  }
  await expect(page.getByRole("button", { name: "Send", exact: true })).toBeVisible();

  await page.getByRole("button", { name: "Clear", exact: true }).click();
  await expect(column(page, mini).getByRole("group", { name: "Prompt" })).toHaveCount(0);
  await prompt.fill("Tell me more [mock:gen=4000] [mock:tps=40]");
  await page.getByRole("button", { name: "Send", exact: true }).click();
  await expect(page.getByRole("button", { name: "Stop", exact: true })).toBeVisible();
  await expect(lastPrompt(page, mini)).toHaveText(/^Tell me more/);
  for (const name of [mini, nano]) await expect(reply(page, name).getByRole("paragraph").first()).not.toBeEmpty();
  await page.getByRole("button", { name: "Stop", exact: true }).click();
  for (const name of [mini, nano]) await expect(reply(page, name).getByText("Stopped", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Send", exact: true })).toBeVisible();

  await page.reload();
  await expect(column(page, mini)).toBeVisible();
  await expect(column(page, nano)).toBeVisible();
});

test("a refusal shows the gateway's message in its column", async ({ page }) => {
  await signInAsOwner(page);
  await page.goto(`/orgs/${org}/projects/${project}/playground?models=openai/gpt-5.4-mini`);
  await page.getByRole("textbox", { name: "Prompt" }).fill("Fail [mock:error=server-error]");
  await page.getByRole("button", { name: "Send", exact: true }).click();
  const failure = reply(page, /^GPT-5\.4 mini$/).getByRole("alert");
  await expect(failure.getByText("No response", { exact: true })).toBeVisible();
  await expect(failure.getByText(/provider|try again/i)).toBeVisible();
  await expect(page.getByRole("region", { name: "Notifications" }).getByText(/provider|try again/i)).toHaveCount(0);
});

test("playground requests are in the request log with a Playground tag and filter by source", async ({ page }) => {
  await signInAsOwner(page);
  await page.goto(`/orgs/${org}/projects/${project}/logs`);
  const rows = page
    .getByRole("table", { name: "Requests" })
    .getByRole("row")
    .filter({ hasNot: page.getByRole("columnheader"), has: page.locator("time") });
  await expect(async () => {
    await page.reload();
    await expect(rows.first()).toBeVisible({ timeout: 2000 });
  }).toPass({ timeout: 30_000 });
  await expect(rows.first().getByText("Playground", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Add filter" }).click();
  await page.getByRole("menuitem", { name: "Source" }).click();
  await page.getByRole("menuitemradio", { name: "API" }).click();
  await expect(page.getByText("No requests match these filters")).toBeVisible();
  await expect(page).toHaveURL(/source=api/);
});

test("a viewer has no playground: no sidebar item, and its address says which right is missing", async ({ page, browser }) => {
  await clearMail();
  await signInAsOwner(page);
  const headers = { origin: new URL(page.url()).origin };
  const invited = await page.request.post(`/platform/v1/orgs/${org}/invites`, { headers, data: { email: VIEWER.email, role: "member" } });
  expect(invited.ok()).toBe(true);
  const other = await browser.newContext();
  const viewer = await other.newPage();
  await viewer.goto(await mailLink(VIEWER.email, /\/invite/));
  await viewer.getByLabel("Name").fill(VIEWER.name);
  await viewer.getByLabel("Password", { exact: true }).fill(NEW_PASSWORD);
  await viewer.getByRole("button", { name: "Create account and join" }).click();
  await expectSignedIn(viewer);
  const members = (await (await page.request.get(`/platform/v1/orgs/${org}/members?limit=100`)).json()) as { data: { user: { id: string; email: string } }[] };
  const id = members.data.find((m) => m.user.email === VIEWER.email)?.user.id;
  const added = await page.request.post(`/platform/v1/orgs/${org}/projects/${project}/members`, { headers, data: { user_id: id, role: "viewer" } });
  expect(added.ok()).toBe(true);

  await viewer.goto(`/orgs/${org}/projects/${project}/logs`);
  const nav = viewer.getByRole("navigation", { name: "Console" });
  await expect(nav.getByRole("link", { name: "Request logs" })).toBeVisible();
  await expect(nav.getByRole("link", { name: "Playground" })).toHaveCount(0);
  await viewer.goto(`/orgs/${org}/projects/${project}/playground`);
  await expect(toastText(viewer, /^You need Use the Playground; ask .+ for it\.$/)).toBeVisible();
  await expect(viewer).not.toHaveURL(/\/playground/);
  await other.close();
});

test("tapping the prompt on a phone does not zoom the page", async ({ page, browser }) => {
  await signInAsOwner(page);
  const origin = new URL(page.url()).origin;
  const context = await browser.newContext({
    viewport: { width: 390, height: 844 },
    hasTouch: true,
    isMobile: true,
    storageState: await page.context().storageState(),
  });
  const phone = await context.newPage();
  await phone.goto(`${origin}/orgs/${org}/projects/${project}/playground?models=openai/gpt-5.4-mini`);
  const prompt = phone.getByRole("textbox", { name: "Prompt" });
  await prompt.tap();
  await expect(prompt).toBeFocused();
  await expect(prompt).toHaveCSS("font-size", "16px");
  await context.close();
});

test("the playground's own controls are in the reader's language", async ({ page, browser }) => {
  await signInAsOwner(page);
  const origin = new URL(page.url()).origin;
  const context = await browser.newContext({ locale: "de-DE", storageState: await page.context().storageState() });
  const german = await context.newPage();
  await german.goto(`${origin}/orgs/${org}/projects/${project}/playground?models=openai/gpt-5.4-mini`);
  await expect(german.getByRole("button", { name: "Senden", exact: true })).toBeVisible();
  await german.getByRole("button", { name: "Parameter", exact: true }).click();
  await expect(german.getByRole("dialog", { name: "Parameter" }).getByRole("textbox", { name: "Systemanweisungen" })).toBeVisible();
  await context.close();
});
