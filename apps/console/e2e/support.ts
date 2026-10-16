import { test as base, expect, type Browser, type BrowserContext, type Cookie, type Locator, type Page } from "@playwright/test";
import { STACK_URLS } from "./stack";

export { expect };

const FAILED_REQUEST = /^Failed to load resource: the server responded with a status of \d{3}\b/;

const REPORT = "__kaesoE2eBrowserError";
type ErrorGlobals = {
  addEventListener(type: "error", listener: (e: { error: unknown; message: string }) => void): void;
} & Record<string, ((message: string) => void) | undefined>;

export const test = base.extend<{ browserErrors: undefined }>({
  browserErrors: [
    async ({ browser, context }, use) => {
      const errors = new Map<string, { count: number; at: string }>();
      const report = (page: Page, error: string) => {
        const seen = errors.get(error);
        if (seen) seen.count++;
        else errors.set(error, { count: 1, at: page.url().replace(/#.*/, "") });
      };
      const watchPage = (page: Page) => {
        page.on("pageerror", (e) => report(page, `uncaught ${e.name}: ${e.message}`));
        page.on("console", (m) => {
          if (m.type() === "error" && !FAILED_REQUEST.test(m.text())) report(page, `console.error: ${m.text()}`);
        });
      };
      const watch = async (c: BrowserContext) => {
        await c.exposeBinding(REPORT, ({ page }, message: string) => report(page, `error event: ${message}`));
        await c.addInitScript((binding) => {
          const g = globalThis as unknown as ErrorGlobals;
          g.addEventListener("error", (e) => {
            if (!e.error) g[binding]?.(e.message);
          });
        }, REPORT);
        c.pages().forEach(watchPage);
        c.on("page", watchPage);
      };
      await watch(context);
      const newContext = browser.newContext;
      browser.newContext = async (...args: Parameters<Browser["newContext"]>) => {
        const c = await newContext.apply(browser, args);
        await watch(c);
        return c;
      };
      try {
        await use(undefined);
      } finally {
        browser.newContext = newContext;
      }
      if (errors.size) {
        const lines = [...errors].map(([error, { count, at }]) => `${error} (${count === 1 ? "" : `${count} times, first `}at ${at})`);
        throw new Error(`The browser reported ${errors.size === 1 ? "an error" : `${errors.size} errors`}:\n${lines.join("\n")}`);
      }
    },
    { auto: true },
  ],
});

export const OWNER = { name: "Ada Okafor", email: "owner@example.com", org: "Northwind Labs" };
export const PASSWORD = "correct horse battery staple";
export const NEW_PASSWORD = "a different horse battery";

let ownerCookies: Cookie[] | null = null;

export async function signInAsOwner(page: Page): Promise<void> {
  if (ownerCookies) {
    await page.context().addCookies(ownerCookies);
    await page.goto("/");
    const shell = page.getByRole("navigation", { name: "Console" });
    const signIn = page.getByRole("heading", { name: "Sign in", exact: true });
    await expect(shell.or(signIn)).toBeVisible();
    if (await shell.isVisible()) return;
  }
  await signInWithPassword(page, OWNER.email, NEW_PASSWORD);
  await expectSignedIn(page);
  ownerCookies = await page.context().cookies();
}

export async function openDefaultProject(page: Page): Promise<void> {
  const nav = page.getByRole("navigation", { name: "Console" });
  await nav.waitFor();
  const memberships = (await (await page.request.get("/platform/v1/me/memberships")).json()) as {
    data: { org_id: string; projects: { id: string; is_default: boolean }[] }[];
  };
  const org = new URL(page.url()).pathname.split("/")[2] ?? memberships.data[0]?.org_id;
  const home = memberships.data.find((m) => m.org_id === org) ?? memberships.data[0];
  const project = home?.projects.find((p) => p.is_default)?.id;
  await nav.locator(`a[href="/orgs/${home?.org_id}/projects/${project}"]`).click();
  await expect(page).toHaveURL(/\/orgs\/[^/]+\/projects\/[^/]+$/);
}

export async function signInToProject(page: Page): Promise<void> {
  await signInAsOwner(page);
  await openDefaultProject(page);
}

const mailSearch = (query: string) => `${STACK_URLS.mail}/api/v1/search?query=${encodeURIComponent(query)}`;

export async function mailLink(to: string, path: RegExp): Promise<string> {
  for (let i = 0; i < 60; i++) {
    const list = (await (await fetch(mailSearch(`to:${to}`))).json()) as { messages?: { ID: string }[] };
    for (const m of list.messages ?? []) {
      const msg = (await (await fetch(`${STACK_URLS.mail}/api/v1/message/${m.ID}`)).json()) as { Text?: string };
      const url = new RegExp(`https?://[^\\s]*${path.source}[^\\s]*`).exec(msg.Text ?? "")?.[0];
      if (url) return url;
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error(`no email to ${to} with ${path}`);
}

export async function clearMail(): Promise<void> {
  await fetch(`${STACK_URLS.mail}/api/v1/messages`, { method: "DELETE" });
}

export async function enterEmail(page: Page, email: string): Promise<void> {
  await page.getByLabel("Email").fill(email);
  await page.getByRole("button", { name: "Continue", exact: true }).click();
}

export async function signInWithPassword(page: Page, email: string, password: string): Promise<void> {
  await page.goto("/sign-in");
  await enterEmail(page, email);
  await page.getByLabel("Password", { exact: true }).fill(password);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
}

export async function expectSignedIn(page: Page): Promise<void> {
  await expect(page.getByRole("navigation", { name: "Console" })).toBeVisible();
}

export const toastText = (page: Page, text: string | RegExp) => page.getByRole("region", { name: "Notifications" }).getByText(text).first();

export async function stepTo(dialog: Locator, target: Locator): Promise<void> {
  const step = dialog.getByText(/^Step \d+ of \d+$/);
  await expect(step).toBeVisible();
  for (let i = 0; i < 8 && !(await target.isVisible()); i++) {
    const at = (await step.textContent()) ?? "";
    await dialog.getByRole("button", { name: "Continue", exact: true }).click();
    await expect(step).not.toHaveText(at);
  }
  await expect(target).toBeVisible();
}

export async function submitSteps(dialog: Locator, submit: string): Promise<void> {
  const button = dialog.getByRole("button", { name: submit, exact: true });
  await stepTo(dialog, button);
  await button.click();
}

interface SavedDir {
  getFileHandle(name: string, options?: { create: boolean }): Promise<{ getFile(): Promise<{ text(): Promise<string> }> }>;
  keys(): AsyncIterable<string>;
  removeEntry(name: string): Promise<void>;
}
type PageGlobals = { navigator: { storage: { getDirectory(): Promise<SavedDir> } }; showSaveFilePicker?: unknown };

export async function answerSaveDialog(page: Page): Promise<void> {
  await page.evaluate(() => {
    const g = globalThis as unknown as PageGlobals;
    g.showSaveFilePicker = async (o: { suggestedName: string }) => (await g.navigator.storage.getDirectory()).getFileHandle(o.suggestedName, { create: true });
  });
}

export function savedFiles(page: Page): Promise<Record<string, string>> {
  return page.evaluate(async () => {
    const dir = await (globalThis as unknown as PageGlobals).navigator.storage.getDirectory();
    const out: Record<string, string> = {};
    for await (const name of dir.keys()) out[name] = await (await (await dir.getFileHandle(name)).getFile()).text();
    return out;
  });
}

export async function removeSaved(page: Page, name: string): Promise<void> {
  await page.evaluate(async (n) => (await (globalThis as unknown as PageGlobals).navigator.storage.getDirectory()).removeEntry(n), name);
}

export async function withoutSaveDialog(page: Page): Promise<void> {
  await page.evaluate(() => delete (globalThis as unknown as PageGlobals).showSaveFilePicker);
}
