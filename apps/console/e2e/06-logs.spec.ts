import type { Page } from "@playwright/test";
import { expect, test, answerSaveDialog, removeSaved, savedFiles, signInToProject, toastText, withoutSaveDialog } from "./support";

test.describe.configure({ mode: "serial" });

const orgOf = (page: Page) => /\/orgs\/([^/]+)/.exec(new URL(page.url()).pathname)?.[1] ?? "";
const projectOf = (page: Page) => /\/projects\/([^/]+)/.exec(new URL(page.url()).pathname)?.[1] ?? "";
const rows = (page: Page) =>
  page
    .getByRole("table", { name: "Requests" })
    .getByRole("row")
    .filter({ hasNot: page.getByRole("columnheader"), has: page.locator("time") });

test("requests through the gateway appear in the log, filter, and open in the inspector", async ({ page }) => {
  await signInToProject(page);
  const origin = new URL(page.url()).origin;
  const org = orgOf(page);
  const project = projectOf(page);
  const headers = { origin };
  expect(
    (
      await page.request.post(`/platform/v1/orgs/${org}/credentials`, {
        headers,
        data: { provider: "openai", name: "Mock", secret: "sk-proj-mock-0000000000000000openai" },
      })
    ).ok(),
  ).toBe(true);
  const created = await page.request.post(`/platform/v1/orgs/${org}/projects/${project}/keys`, { headers, data: { name: "logs-test", access: "inference" } });
  expect(created.ok()).toBe(true);
  const { secret } = (await created.json()) as { secret: string };
  const call = (content: string) =>
    page.request.post("/v1/chat/completions", {
      headers: { authorization: `Bearer ${secret}` },
      data: { model: "gpt-5.4-mini", messages: [{ role: "user", content }] },
    });
  expect((await call("Where is order 7?")).status()).toBe(200);
  const failed = (await call("Again [mock:error=server-error]")).status();
  expect(failed).toBeGreaterThanOrEqual(400);

  await page.getByRole("link", { name: "Request logs" }).click();
  await expect(page).toHaveTitle(/^Request logs · /);
  await expect(async () => {
    await page.reload();
    await expect(rows(page)).toHaveCount(2, { timeout: 2000 });
  }).toPass({ timeout: 30_000 });

  await page.getByRole("button", { name: "Add filter" }).click();
  await page.getByRole("menuitem", { name: "Failed only" }).click();
  await expect(rows(page)).toHaveCount(1);
  await expect(rows(page).first().getByText(String(failed), { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Remove the Failed only filter" }).click();
  await expect(rows(page)).toHaveCount(2);

  await rows(page).filter({ hasText: "200" }).click();
  const sheet = page.getByRole("dialog", { name: /chat\/completions · gpt-5\.4[ -]mini/i });
  await expect(sheet.getByText("OpenAI Chat Completions")).toBeVisible();
  await expect(sheet.getByText("logs-test")).toBeVisible();
  await sheet.getByRole("tab", { name: "Request" }).click();
  await expect(sheet.getByText("Where is order 7?")).toBeVisible();
  await sheet.getByRole("tab", { name: "Reply" }).click();
  await expect(sheet.getByText("End of turn")).toBeVisible();
  await sheet.getByRole("tab", { name: "Overview" }).click();

  await page.keyboard.press("k");
  await expect(page.getByRole("dialog", { name: new RegExp(String(failed)) })).toBeVisible();
  await page.keyboard.press("Escape");

  await page.setViewportSize({ width: 375, height: 812 });
  await expect(page.getByRole("columnheader", { name: "Status" })).toBeVisible();
  await expect(rows(page).first().getByText(String(failed), { exact: true })).toBeVisible();
});

test("a streamed tool call opens in the inspector, a key's rate limit shows as 429, and the log exports", async ({ page }) => {
  await signInToProject(page);
  const origin = new URL(page.url()).origin;
  const org = orgOf(page);
  const project = projectOf(page);
  const headers = { origin };
  const created = await page.request.post(`/platform/v1/orgs/${org}/projects/${project}/keys`, { headers, data: { name: "stream-test", access: "inference" } });
  const { id: keyId, secret } = (await created.json()) as { id: string; secret: string };
  const streamed = await page.request.post("/v1/chat/completions", {
    headers: { authorization: `Bearer ${secret}` },
    data: {
      model: "gpt-5.4-mini",
      stream: true,
      messages: [{ role: "user", content: "What's the weather in Paris?" }],
      tools: [{ type: "function", function: { name: "get_weather", parameters: { type: "object", properties: { city: { type: "string" } } } } }],
    },
  });
  expect(streamed.status()).toBe(200);

  expect(
    (
      await page.request.post(`/platform/v1/orgs/${org}/projects/${project}/limits`, {
        headers,
        data: { level: "key", key: keyId, kind: "request_rate", amount: "1", period: "minute" },
      })
    ).ok(),
  ).toBe(true);
  const again = () =>
    page.request.post("/v1/chat/completions", {
      headers: { authorization: `Bearer ${secret}` },
      data: { model: "gpt-5.4-mini", messages: [{ role: "user", content: "Again" }] },
    });
  expect((await again()).status()).toBe(200);
  expect((await again()).status()).toBe(429);

  await page.goto(`/orgs/${org}/projects/${project}/logs?key=${keyId}`);
  await expect(async () => {
    await page.reload();
    await expect(rows(page)).toHaveCount(3, { timeout: 2000 });
  }).toPass({ timeout: 30_000 });
  await expect(rows(page).filter({ hasText: "429" })).toHaveCount(1);

  await rows(page).last().click();
  const sheet = page.getByRole("dialog", { name: /chat\/completions · gpt-5\.4[ -]mini/i });
  await expect(sheet.getByText("Streamed")).toBeVisible();
  await sheet.getByRole("tab", { name: "Reply" }).click();
  await expect(sheet.getByText(/Assembled from the stream/)).toBeVisible();
  await expect(sheet.getByText("get_weather").first()).toBeVisible();
  await page.keyboard.press("Escape");

  const exportButton = page.getByRole("button", { name: "Export", exact: true }).first();
  const dialog = page.getByRole("dialog", { name: "Export requests" });
  const isExport = (url: URL) => url.pathname.endsWith("/logs/export");
  const lines = (text: string) => text.trim().split("\n");
  await exportButton.click();
  await expect(dialog.getByText("Every request from the last 24 hours with key stream-test, newest first.")).toBeVisible();
  await dialog.getByRole("button", { name: "Cancel" }).click();
  await expect(dialog).toBeHidden();

  await answerSaveDialog(page);
  await exportButton.click();
  await dialog
    .getByRole("radio", { name: /^JSON Lines/ })
    .first()
    .click();
  await dialog.getByRole("button", { name: "Export", exact: true }).click();
  await expect(toastText(page, "Requests exported")).toBeVisible();
  const saved = await savedFiles(page);
  expect(Object.keys(saved)).toEqual([`kaeso-logs-${project}.jsonl`]);
  const exported = lines(saved[`kaeso-logs-${project}.jsonl`] ?? "").map((l) => JSON.parse(l) as { key_id: string });
  expect(exported).toHaveLength(3);
  expect(exported.every((r) => r.key_id === keyId)).toBe(true);
  await removeSaved(page, `kaeso-logs-${project}.jsonl`);

  let release = () => {};
  await page.route(isExport, async (route) => {
    await new Promise<void>((resolve) => (release = resolve));
    await route.abort().catch(() => {});
  });
  await exportButton.click();
  await dialog.getByRole("button", { name: "Export", exact: true }).click();
  await expect(toastText(page, "Exporting requests")).toBeVisible();
  await exportButton.click();
  await expect(dialog).toBeHidden();
  await page.getByRole("region", { name: "Notifications" }).getByRole("button", { name: "Cancel" }).click();
  await expect(toastText(page, "Export canceled")).toBeVisible();
  expect(await savedFiles(page)).toEqual({});
  release();
  await page.unroute(isExport);

  await page.route(isExport, (route) =>
    route.fulfill({
      status: 503,
      contentType: "application/json",
      json: { error: { type: "unavailable", message: "Request logs are unavailable right now. Try again." } },
    }),
  );
  await exportButton.click();
  await dialog.getByRole("button", { name: "Export", exact: true }).click();
  await expect(toastText(page, "Request logs are unavailable right now. Try again.")).toBeVisible();
  expect(await savedFiles(page)).toEqual({});
  await page.unroute(isExport);

  await withoutSaveDialog(page);
  await exportButton.click();
  await dialog.getByRole("radio", { name: /^CSV/ }).click();
  const download = page.waitForEvent("download");
  await dialog.getByRole("button", { name: "Export", exact: true }).click();
  const file = await download;
  await expect(toastText(page, "Export started")).toBeVisible();
  expect(file.suggestedFilename()).toBe(`kaeso-logs-${project}.csv`);
  const csv = await (await file.createReadStream()).toArray().then((chunks) => Buffer.concat(chunks).toString("utf8"));
  expect(lines(csv)).toHaveLength(4);

  await page.goto(`/orgs/${org}/projects/${project}/logs?key=${keyId}&status=504`);
  await expect(page.getByText("No requests match these filters")).toBeVisible();
  await expect(exportButton).toBeDisabled();
});

test("a Markdown system prompt reads as Markdown, and Show raw shows it as sent", async ({ page }) => {
  await signInToProject(page);
  const origin = new URL(page.url()).origin;
  const org = orgOf(page);
  const project = projectOf(page);
  const created = await page.request.post(`/platform/v1/orgs/${org}/projects/${project}/keys`, {
    headers: { origin },
    data: { name: "markdown-test", access: "inference" },
  });
  const { id: keyId, secret } = (await created.json()) as { id: string; secret: string };
  const system = "# Harness\n\n- Read the order\n- Answer in `one` line\n\n<system-reminder>Stay calm.</system-reminder>";
  const sent = await page.request.post("/v1/chat/completions", {
    headers: { authorization: `Bearer ${secret}` },
    data: {
      model: "gpt-5.4-mini",
      messages: [
        { role: "system", content: system },
        { role: "user", content: "Where is order 7?" },
      ],
    },
  });
  expect(sent.status()).toBe(200);

  await page.goto(`/orgs/${org}/projects/${project}/logs?key=${keyId}`);
  await expect(async () => {
    await page.reload();
    await expect(rows(page)).toHaveCount(1, { timeout: 2000 });
  }).toPass({ timeout: 30_000 });
  await rows(page).first().click();
  const sheet = page.getByRole("dialog", { name: /chat\/completions · gpt-5\.4[ -]mini/i });
  await sheet.getByRole("tab", { name: "Request" }).click();
  const prompt = sheet.getByRole("region", { name: "System" });
  await expect(prompt.getByRole("heading", { name: "Harness" })).toBeVisible();
  await expect(prompt.getByRole("listitem")).toHaveText(["•Read the order", "•Answer in one line"]);
  await expect(prompt.getByText("<system-reminder>Stay calm.</system-reminder>")).toBeVisible();

  await sheet.getByRole("switch", { name: "Show raw" }).click();
  await expect(sheet.getByRole("heading", { name: "Harness" })).toBeHidden();
  await expect(sheet.getByText(/# Harness\s+- Read the order\s+- Answer in `one` line/)).toBeVisible();
});
