import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ORG, providers } from "~/test/fixtures";
import { hoverReason, notifications } from "~/test/fixtures/org";
import { credential } from "~/test/fixtures/shell";
import { apiError, on, page, requests } from "~/test/network";
import { renderUi } from "~/test/render";
import { ProviderKeyDialog } from "./provider-key-dialog";

const CREDENTIALS = "orgs/:org/credentials";
const AUTHORIZE = "https://auth.openai.com/api/accounts/authorize?client_id=dynamic_agent_client&state=abc";
const CALLBACK = "http://127.0.0.1:51234/auth/callback?code=c-1&state=abc&client_id=oaiapp_1";
const ATTEMPT = { attempt: "019a1b2c-3d4e-7f60-8a9b-0c1d2e3f4a5b", authorize_url: AUTHORIZE, expires_at: "2026-10-23T12:10:00Z" };
const plan = credential({ id: "cred_plan", name: "Ada's plan", auth_method: "chatgpt", account: "ada@example.com", last4: "" });

function newTab() {
  const tab = { opener: window as unknown, location: { href: "" }, close: vi.fn() };
  const open = vi.spyOn(window, "open").mockReturnValue(tab as unknown as Window);
  return { tab, open };
}

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

const GROK_PAGE = "https://accounts.x.ai/oauth2/device";
const grokKey = credential({ id: "cred_grok", name: "Ada's Grok", provider: "xai", auth_method: "grok", account: "ada@example.com", last4: "" });

function grokAttempt(seconds = 600, code = "FKFF-E5EP") {
  return {
    attempt: `019a1b2c-3d4e-7f60-8a9b-0c1d2e3f4a5${code.at(-1)}`,
    authorize_url: null,
    device: { user_code: code, verification_uri: GROK_PAGE, verification_uri_complete: `${GROK_PAGE}?user_code=${code}`, interval: 5 },
    expires_at: new Date(Date.now() + seconds * 1000).toISOString(),
    expires_in: seconds,
  };
}

function answers(...replies: unknown[]) {
  let at = 0;
  return () => replies[Math.min(at++, replies.length - 1)];
}

const later = (ms: number) => act(() => vi.advanceTimersByTimeAsync(ms));

async function getCode(dialog: HTMLElement) {
  vi.useFakeTimers({ now: Date.now(), toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval", "Date"] });
  fireEvent.click(within(dialog).getByRole("button", { name: /^Get a (sign-in|new) code$/ }));
  await later(0);
  await later(0);
}

const readable = (el: Element | null) => el?.textContent?.replace(/\u2060? \(opens in a new tab\)/, "");

function pageHint(dialog: HTMLElement, text: string) {
  const link = within(dialog).getByRole("link", { name: /^accounts\.x\.ai/ });
  const hint = within(dialog).getByText((_, el) => readable(el) === text && ![...(el?.children ?? [])].some((c) => readable(c) === text));
  return { link, hint };
}

function expectKeyHint(dialog: HTMLElement, host: string, url: string, page: string, prefix: string) {
  const link = within(dialog).getByRole("link", { name: (name) => name.startsWith(host) });
  expect(link).toHaveAttribute("href", url);
  expect(link).toHaveAttribute("target", "_blank");
  expect(link.getAttribute("rel")).toContain("noreferrer");
  const hint = link.closest("p") as HTMLElement;
  expect(hint.textContent?.replace(/\u2060? \(opens in a new tab\)/, "")).toBe(`Create one at ${host} under ${page}; it starts with ${prefix}.`);
  expect(link).toHaveAccessibleName(`${host} (opens in a new tab)`);
  expect(within(hint).getByText(prefix)).toHaveClass("text-nowrap");
}

async function choose(user: Awaited<ReturnType<typeof renderUi>>["user"], dialog: HTMLElement, provider: string) {
  await user.click(within(dialog).getByRole("radio", { name: (name) => name.startsWith(provider) }));
}

async function open(props: Partial<Parameters<typeof ProviderKeyDialog>[0]> = {}) {
  const onClose = vi.fn();
  on("GET", "orgs/:org/providers", page(providers));
  const view = await renderUi(<ProviderKeyDialog org={ORG} keys={[]} onClose={onClose} {...props} />);
  const dialog = await screen.findByRole("dialog", { name: /provider key|Replace/ });
  return { ...view, dialog, onClose };
}

describe("ProviderKeyDialog", () => {
  it("adds a key for the provider you pick", async () => {
    on("POST", CREDENTIALS, credential({ provider: "anthropic", name: "Messages prod", is_default: false }));
    const { user, dialog, onClose } = await open({ keys: [credential(), credential({ id: "cred_two", is_default: false })] });

    expect(within(dialog).getByRole("heading", { name: "Add provider key" })).toBeInTheDocument();
    expect(within(within(dialog).getByRole("group", { name: "Connected" })).getByRole("radio")).toHaveAccessibleName("OpenAI 2 keys");
    expect(within(dialog).getByRole("radio", { name: "OpenAI 2 keys" })).toHaveAttribute("aria-checked", "false");
    await hoverReason(user, within(dialog).getByRole("button", { name: "Continue" }), "Choose a provider");
    expect(within(dialog).getByRole("group", { name: "Other providers" })).toContainElement(within(dialog).getByRole("radio", { name: "Anthropic" }));
    await user.click(within(dialog).getByRole("radio", { name: "Anthropic" }));

    expect(await within(dialog).findByRole("heading", { name: "Paste your Anthropic API key" })).toBeVisible();
    expectKeyHint(dialog, "platform.claude.com", "https://platform.claude.com/settings/keys", "Settings → API keys", "sk-ant-");
    const secret = within(dialog).getByLabelText("Anthropic API key");
    expect(secret).toHaveAttribute("placeholder", "sk-ant-…");
    await user.type(secret, "  sk-ant-123  ");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), " Messages prod ");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Anthropic · Messages prod added")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "anthropic", name: "Messages prod", secret: "sk-ant-123", make_default: false },
    ]);
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });

  it("offers only the providers the gateway lists, even when the organization still has a key for another", async () => {
    const { dialog } = await open({ keys: [credential({ id: "cred_cerebras", provider: "cerebras", name: "Cerebras main" })] });
    expect(
      within(dialog)
        .getAllByRole("radio")
        .map((o) => o.textContent),
    ).toHaveLength(providers.length);
    expect(within(dialog).queryByRole("radio", { name: /Cerebras/ })).not.toBeInTheDocument();
  });

  it("says when the providers can't load and loads them again", async () => {
    let calls = 0;
    on("GET", "orgs/:org/providers", () => (++calls === 1 ? apiError(429, "rate_limited", "Too many requests; try again in a minute.") : page(providers)));
    const onClose = vi.fn();
    const { user } = await renderUi(<ProviderKeyDialog org={ORG} keys={[]} onClose={onClose} />);
    const dialog = await screen.findByRole("dialog", { name: /provider key/ });
    expect(within(dialog).queryByRole("radio")).not.toBeInTheDocument();
    expect(await notifications().findByText("Too many requests; try again in a minute.")).toBeInTheDocument();
    await user.click(notifications().getByRole("button", { name: "Try again", hidden: true }));
    expect(await within(dialog).findAllByRole("radio")).toHaveLength(providers.length);
  });

  it("adds a DeepSeek key", async () => {
    on("POST", CREDENTIALS, credential({ provider: "deepseek", name: "Reasoning", is_default: true }));
    const { user, dialog } = await open();

    await user.click(within(dialog).getByRole("radio", { name: "DeepSeek" }));

    expect(await within(dialog).findByRole("heading", { name: "Paste your DeepSeek API key" })).toBeVisible();
    expectKeyHint(dialog, "platform.deepseek.com", "https://platform.deepseek.com/api_keys", "API keys", "sk-");
    const secret = within(dialog).getByLabelText("DeepSeek API key");
    expect(secret).toHaveAttribute("placeholder", "sk-…");
    await user.type(secret, "sk-deepseek-123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Reasoning");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Reasoning is the DeepSeek default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "deepseek", name: "Reasoning", secret: "sk-deepseek-123", make_default: false },
    ]);
  });

  it("adds a SpaceX key with the xAI console as its key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "xai", name: "Grok prod", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "SpaceX");
    expect(within(dialog).getByRole("heading", { name: "Add an API key or sign in with Grok?" })).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));

    expect(await within(dialog).findByRole("heading", { name: "Paste your SpaceX API key" })).toBeVisible();
    expectKeyHint(dialog, "console.x.ai", "https://console.x.ai/team/default/api-keys", "API Keys", "xai-");
    const secret = within(dialog).getByLabelText("SpaceX API key");
    expect(secret).toHaveAttribute("placeholder", "xai-…");
    await user.type(secret, "xai-123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Grok prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Grok prod is the SpaceX default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([{ provider: "xai", name: "Grok prod", secret: "xai-123", make_default: false }]);
  });

  it("adds a Together AI key", async () => {
    on("POST", CREDENTIALS, credential({ provider: "togetherai", name: "Open models", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "Together AI");

    expect(await within(dialog).findByRole("heading", { name: "Paste your Together AI API key" })).toBeVisible();
    expectKeyHint(dialog, "api.together.ai", "https://api.together.ai/settings/projects/~current/api-keys", "API Keys", "tgp_v1_");
    const secret = within(dialog).getByLabelText("Together AI API key");
    expect(secret).toHaveAttribute("placeholder", "tgp_v1_…");
    await user.type(secret, "tgp_v1_123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Open models");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Open models is the Together AI default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "togetherai", name: "Open models", secret: "tgp_v1_123", make_default: false },
    ]);
  });

  it("adds a Groq key with Groq's own key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "groq", name: "Groq prod", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "Groq");

    expect(await within(dialog).findByRole("heading", { name: "Paste your Groq API key" })).toBeVisible();
    expectKeyHint(dialog, "console.groq.com", "https://console.groq.com/keys", "API Keys", "gsk_");
    const secret = within(dialog).getByLabelText("Groq API key");
    expect(secret).toHaveAttribute("placeholder", "gsk_…");
    await user.type(secret, "gsk_123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Groq prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Groq prod is the Groq default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([{ provider: "groq", name: "Groq prod", secret: "gsk_123", make_default: false }]);
  });

  it("adds an OpenRouter key with OpenRouter's own key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "openrouter", name: "OpenRouter prod", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "OpenRouter");

    expect(await within(dialog).findByRole("heading", { name: "Paste your OpenRouter API key" })).toBeVisible();
    expectKeyHint(dialog, "openrouter.ai", "https://openrouter.ai/settings/keys", "API Keys", "sk-or-v1-");
    const secret = within(dialog).getByLabelText("OpenRouter API key");
    expect(secret).toHaveAttribute("placeholder", "sk-or-v1-…");
    await user.type(secret, "sk-or-v1-123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "OpenRouter prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("OpenRouter prod is the OpenRouter default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "openrouter", name: "OpenRouter prod", secret: "sk-or-v1-123", make_default: false },
    ]);
  });

  it("adds a Vercel key with Vercel's own key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "vercel", name: "Gateway", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "Vercel");

    expect(await within(dialog).findByRole("heading", { name: "Paste your Vercel API key" })).toBeVisible();
    expectKeyHint(dialog, "vercel.com", "https://vercel.com/d?to=%2F%5Bteam%5D%2F%7E%2Fai-gateway%2Fapi-keys", "AI Gateway → API Keys", "vck_");
    const secret = within(dialog).getByLabelText("Vercel API key");
    expect(secret).toHaveAttribute("placeholder", "vck_…");
    await user.type(secret, "vck_123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Gateway");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Gateway is the Vercel default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([{ provider: "vercel", name: "Gateway", secret: "vck_123", make_default: false }]);
  });

  it("says what the key step still needs", async () => {
    const { user, dialog } = await open();
    await choose(user, dialog, "Anthropic");
    const secret = within(dialog).getByLabelText("Anthropic API key");
    const add = () => within(dialog).getByRole("button", { name: "Add key" });
    await hoverReason(user, add(), "Paste the Anthropic API key");
    await user.type(secret, "sk-ant one");
    await hoverReason(user, add(), "The key can't contain spaces or line breaks");
    await user.clear(secret);
    await user.type(secret, "sk-ant-123");
    await hoverReason(user, add(), "Give the key a name");
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(requests("POST", CREDENTIALS)).toHaveLength(0);
  });

  it("adds another key for a connected provider and can make it the default", async () => {
    on("POST", CREDENTIALS, credential({ name: "Backup", is_default: true }));
    const { user, dialog } = await open({ keys: [credential()] });

    await choose(user, dialog, "OpenAI");
    expect(within(dialog).getByText("Step 2 of 4")).toBeInTheDocument();
    expect(within(dialog).getByRole("heading", { name: "Add an API key or sign in with ChatGPT?" })).toBeInTheDocument();
    expect(within(dialog).getByRole("radio", { name: /^API key/ })).toHaveAttribute("aria-checked", "true");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(within(dialog).getByText("Step 3 of 4")).toBeInTheDocument();
    expectKeyHint(dialog, "platform.openai.com", "https://platform.openai.com/api-keys", "API keys", "sk-");
    await user.type(within(dialog).getByLabelText("OpenAI API key"), "sk-new");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Backup{Enter}");

    expect(await within(dialog).findByRole("heading", { name: "Make it the OpenAI default?" })).toBeVisible();
    expect(within(dialog).getByRole("radio", { name: /Keep Production as the default/ })).toHaveAttribute("aria-checked", "true");
    await user.click(within(dialog).getByRole("radio", { name: /Make this key the default/ }));
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Backup is the OpenAI default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([{ provider: "openai", name: "Backup", secret: "sk-new", make_default: true }]);
  });

  it("adds a Mistral key, which has no fixed prefix, with Mistral's console as its key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "mistral", name: "Mistral prod", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "Mistral");

    expect(await within(dialog).findByRole("heading", { name: "Paste your Mistral API key" })).toBeVisible();
    const link = within(dialog).getByRole("link", { name: (name) => name.startsWith("console.mistral.ai") });
    expect(link).toHaveAttribute("href", "https://console.mistral.ai/api-keys");
    expect((link.closest("p") as HTMLElement).textContent?.replace(/\u2060? \(opens in a new tab\)/, "")).toBe(
      "Create one at console.mistral.ai under API keys.",
    );
    const secret = within(dialog).getByLabelText("Mistral API key");
    expect(secret).not.toHaveAttribute("placeholder");
    await user.type(secret, "AbCdEf0123456789AbCdEf0123456789");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Mistral prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Mistral prod is the Mistral default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "mistral", name: "Mistral prod", secret: "AbCdEf0123456789AbCdEf0123456789", make_default: false },
    ]);
  });

  it("adds a Cohere key, which has no fixed prefix, with Cohere's dashboard as its key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "cohere", name: "Cohere prod", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "Cohere");

    expect(await within(dialog).findByRole("heading", { name: "Paste your Cohere API key" })).toBeVisible();
    const link = within(dialog).getByRole("link", { name: "dashboard.cohere.com (opens in a new tab)" });
    expect(link).toHaveAttribute("href", "https://dashboard.cohere.com/api-keys");
    expect((link.closest("p") as HTMLElement).textContent?.replace(/\u2060? \(opens in a new tab\)/, "")).toBe(
      "Create one at dashboard.cohere.com under API Keys.",
    );
    const secret = within(dialog).getByLabelText("Cohere API key");
    expect(secret).not.toHaveAttribute("placeholder");
    await user.type(secret, "AbCdEf0123456789AbCdEf0123456789AbCdEf01");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Cohere prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Cohere prod is the Cohere default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "cohere", name: "Cohere prod", secret: "AbCdEf0123456789AbCdEf0123456789AbCdEf01", make_default: false },
    ]);
  });

  it("asks only for the provider's key for a provider it has no hint for", async () => {
    const cerebras = credential({ id: "cred_cerebras", provider: "cerebras", name: "Cerebras main" });
    const { dialog } = await open({ keys: [cerebras], replace: cerebras });
    expect(within(dialog).getByRole("heading", { name: "Paste the new cerebras key" })).toBeInTheDocument();
    expect(within(dialog).getByText("Paste the provider's API key.")).toBeInTheDocument();
    expect(within(dialog).getByLabelText("cerebras API key")).not.toHaveAttribute("placeholder");
  });

  it("links a provider's key page without a prefix when its keys have none", async () => {
    const mistral = credential({ id: "cred_mistral", provider: "mistral", name: "Mistral main" });
    const { dialog } = await open({ keys: [mistral], replace: mistral });
    const link = within(dialog).getByRole("link", { name: /console\.mistral\.ai/ });
    expect(link).toHaveAttribute("href", "https://console.mistral.ai/api-keys");
    expect(link.closest("p")?.textContent?.replace(/\u2060? \(opens in a new tab\)/, "")).toBe("Create one at console.mistral.ai under API keys.");
  });

  it("replaces a key's secret", async () => {
    on("POST", `${CREDENTIALS}/:credential/replace`, credential());
    const { user, dialog, onClose } = await open({ keys: [credential()], replace: credential() });

    expect(within(dialog).getByRole("heading", { name: "Replace OpenAI · Production" })).toBeInTheDocument();
    expect(within(dialog).getByRole("heading", { name: "Paste the new OpenAI key" })).toBeInTheDocument();
    expect(within(dialog).queryByRole("textbox", { name: "Name" })).not.toBeInTheDocument();
    await user.type(within(dialog).getByLabelText("OpenAI API key"), "sk-rotated");
    await user.keyboard("{Meta>}{Enter}{/Meta}");

    expect(await notifications().findByText("OpenAI · Production replaced")).toBeInTheDocument();
    expect(requests("POST", `${CREDENTIALS}/:credential/replace`).map((r) => [r.params.credential, r.body])).toEqual([["cred_prod", { secret: "sk-rotated" }]]);
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });

  it("shows the API's message and keeps the dialog open when the key is refused", async () => {
    on("POST", CREDENTIALS, apiError(422, "invalid_request", "OpenAI rejected this key.", { param: "secret" }));
    const { user, dialog, onClose } = await open();

    await choose(user, dialog, "OpenAI");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.type(within(dialog).getByLabelText("OpenAI API key"), "sk-bad");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Bad{Enter}");
    expect((await notifications().findAllByText("OpenAI rejected this key.")).length).toBeGreaterThan(0);
    expect(onClose).not.toHaveBeenCalled();
  });

  it("signs in to a ChatGPT account as a new OpenAI key", async () => {
    const { tab, open: opened } = newTab();
    on("POST", `${CREDENTIALS}/oauth`, ATTEMPT);
    on("POST", `${CREDENTIALS}/oauth/:attempt`, plan);
    const { user, dialog, onClose } = await open();

    await choose(user, dialog, "OpenAI");
    expect(within(dialog).getByText("Requests use the plan of the ChatGPT account you sign in with.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("radio", { name: /^ChatGPT account/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));

    expect(await within(dialog).findByRole("heading", { name: "Sign in with your ChatGPT account" })).toBeVisible();
    expect(within(dialog).getByText("Use your own account only; sharing it breaks OpenAI's terms.")).toBeInTheDocument();
    expect(within(dialog).getByText("Step 3 of 3")).toBeInTheDocument();
    const add = () => within(dialog).getByRole("button", { name: "Add account" });
    await hoverReason(user, add(), "Give the key a name");
    await hoverReason(user, within(dialog).getByRole("button", { name: "Continue with ChatGPT" }), "Give the key a name");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), " Ada's plan ");
    await hoverReason(user, add(), "Continue with ChatGPT first");
    expect(within(dialog).queryByRole("textbox", { name: "Address of the page" })).not.toBeInTheDocument();

    await user.click(within(dialog).getByRole("button", { name: "Continue with ChatGPT" }));
    await waitFor(() => expect(tab.location.href).toBe(AUTHORIZE));
    expect(opened).toHaveBeenCalledWith("about:blank", "_blank");
    expect(tab.opener).toBeNull();
    expect(requests("POST", `${CREDENTIALS}/oauth`).map((r) => r.body)).toEqual([{ provider: "openai", name: "Ada's plan", make_default: false }]);
    expect(within(dialog).queryByRole("link", { name: /Open the ChatGPT sign-in page/ })).not.toBeInTheDocument();
    const pasted = within(dialog).getByRole("textbox", { name: "Address of the page" });
    expect(pasted).toHaveAccessibleDescription("After you approve, the page can't load. That's expected. Paste its address here.");
    await hoverReason(user, add(), "Paste the address of the page that didn't load");
    await user.type(pasted, ` ${CALLBACK} `);
    await user.click(add());

    expect(await notifications().findByText("Ada's plan is the OpenAI default")).toBeInTheDocument();
    expect(requests("POST", `${CREDENTIALS}/oauth/:attempt`).map((r) => [r.params.attempt, r.body])).toEqual([[ATTEMPT.attempt, { callback_url: CALLBACK }]]);
    expect(requests("POST", CREDENTIALS)).toHaveLength(0);
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });

  it("asks about the default before the sign-in and starts over when the name changes", async () => {
    newTab();
    on("POST", `${CREDENTIALS}/oauth`, ATTEMPT);
    on("POST", `${CREDENTIALS}/oauth/:attempt`, { ...plan, is_default: false });
    const { user, dialog } = await open({ keys: [credential()] });

    await choose(user, dialog, "OpenAI");
    await user.click(within(dialog).getByRole("radio", { name: /^ChatGPT account/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByRole("heading", { name: "Make it the OpenAI default?" })).toBeVisible();
    await user.click(within(dialog).getByRole("radio", { name: /Make this key the default/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByText("Step 4 of 4")).toBeInTheDocument();

    const name = within(dialog).getByRole("textbox", { name: "Name" });
    await user.type(name, "Plan");
    await user.click(within(dialog).getByRole("button", { name: "Continue with ChatGPT" }));
    expect(await within(dialog).findByRole("textbox", { name: "Address of the page" })).toBeInTheDocument();
    await user.type(name, "s");
    expect(within(dialog).queryByRole("textbox", { name: "Address of the page" })).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Continue with ChatGPT" }));
    await user.type(await within(dialog).findByRole("textbox", { name: "Address of the page" }), CALLBACK);
    await user.keyboard("{Meta>}{Enter}{/Meta}");

    expect(await notifications().findByText("OpenAI · Ada's plan added")).toBeInTheDocument();
    expect(requests("POST", `${CREDENTIALS}/oauth`).map((r) => r.body)).toEqual([
      { provider: "openai", name: "Plan", make_default: true },
      { provider: "openai", name: "Plans", make_default: true },
    ]);
  });

  it("starts the sign-in over when the default choice changes after it began", async () => {
    newTab();
    on("POST", `${CREDENTIALS}/oauth`, ATTEMPT);
    on("POST", `${CREDENTIALS}/oauth/:attempt`, plan);
    const { user, dialog } = await open({ keys: [credential()] });

    await choose(user, dialog, "OpenAI");
    await user.click(within(dialog).getByRole("radio", { name: /^ChatGPT account/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByRole("heading", { name: "Make it the OpenAI default?" })).toBeVisible();
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.type(await within(dialog).findByRole("textbox", { name: "Name" }), "Plan");
    await user.click(within(dialog).getByRole("button", { name: "Continue with ChatGPT" }));
    expect(await within(dialog).findByRole("textbox", { name: "Address of the page" })).toBeInTheDocument();

    await user.click(within(dialog).getByRole("button", { name: "Back" }));
    await user.click(within(dialog).getByRole("radio", { name: /Make this key the default/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByRole("heading", { name: "Sign in with your ChatGPT account" })).toBeVisible();
    expect(within(dialog).queryByRole("textbox", { name: "Address of the page" })).not.toBeInTheDocument();
    await hoverReason(user, within(dialog).getByRole("button", { name: "Add account" }), "Continue with ChatGPT first");
    await user.click(within(dialog).getByRole("button", { name: "Continue with ChatGPT" }));
    await user.type(await within(dialog).findByRole("textbox", { name: "Address of the page" }), CALLBACK);
    await user.click(within(dialog).getByRole("button", { name: "Add account" }));

    expect(await notifications().findByText("Ada's plan is the OpenAI default")).toBeInTheDocument();
    expect(requests("POST", `${CREDENTIALS}/oauth`).map((r) => r.body)).toEqual([
      { provider: "openai", name: "Plan", make_default: false },
      { provider: "openai", name: "Plan", make_default: true },
    ]);
  });

  it("drops a sign-in that was still starting when its name changed", async () => {
    const { tab } = newTab();
    let respond: (attempt: unknown) => void = () => {};
    on("POST", `${CREDENTIALS}/oauth`, () => new Promise((resolve) => (respond = resolve)));
    on("POST", `${CREDENTIALS}/oauth/:attempt`, plan);
    const { user, dialog } = await open();

    await choose(user, dialog, "OpenAI");
    await user.click(within(dialog).getByRole("radio", { name: /^ChatGPT account/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    const name = await within(dialog).findByRole("textbox", { name: "Name" });
    await user.type(name, "Plan");
    await user.click(within(dialog).getByRole("button", { name: "Continue with ChatGPT" }));
    await waitFor(() => expect(requests("POST", `${CREDENTIALS}/oauth`)).toHaveLength(1));
    await user.type(name, " B");
    respond(ATTEMPT);

    await waitFor(() => expect(tab.close).toHaveBeenCalledTimes(1));
    expect(within(dialog).queryByRole("textbox", { name: "Address of the page" })).not.toBeInTheDocument();
    await hoverReason(user, within(dialog).getByRole("button", { name: "Add account" }), "Continue with ChatGPT first");
    const second = { ...ATTEMPT, attempt: "019a1b2c-3d4e-7f60-8a9b-0c1d2e3f4a5c" };
    on("POST", `${CREDENTIALS}/oauth`, second);
    await user.click(within(dialog).getByRole("button", { name: "Continue with ChatGPT" }));
    await user.type(await within(dialog).findByRole("textbox", { name: "Address of the page" }), CALLBACK);
    await user.click(within(dialog).getByRole("button", { name: "Add account" }));

    expect(await notifications().findByText("Ada's plan is the OpenAI default")).toBeInTheDocument();
    expect(requests("POST", `${CREDENTIALS}/oauth`).map((r) => r.body)).toEqual([
      { provider: "openai", name: "Plan", make_default: false },
      { provider: "openai", name: "Plan B", make_default: false },
    ]);
    expect(requests("POST", `${CREDENTIALS}/oauth/:attempt`).map((r) => r.params.attempt)).toEqual([second.attempt]);
    expect(tab.close).toHaveBeenCalledTimes(1);
  });

  it("starts the sign-in over after a detour through an API key changed its name", async () => {
    newTab();
    on("POST", `${CREDENTIALS}/oauth`, ATTEMPT);
    on("POST", `${CREDENTIALS}/oauth/:attempt`, plan);
    const { user, dialog } = await open();

    await choose(user, dialog, "OpenAI");
    await user.click(within(dialog).getByRole("radio", { name: /^ChatGPT account/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.type(await within(dialog).findByRole("textbox", { name: "Name" }), "Plan");
    await user.click(within(dialog).getByRole("button", { name: "Continue with ChatGPT" }));
    await user.type(await within(dialog).findByRole("textbox", { name: "Address of the page" }), CALLBACK);

    await user.click(within(dialog).getByRole("button", { name: "Back" }));
    await user.click(await within(dialog).findByRole("radio", { name: /^API key/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByRole("heading", { name: "Paste your OpenAI API key" })).toBeVisible();
    const name = within(dialog).getByRole("textbox", { name: "Name" });
    await user.clear(name);
    await user.type(name, "Other plan");
    await user.click(within(dialog).getByRole("button", { name: "Back" }));
    await user.click(await within(dialog).findByRole("radio", { name: /^ChatGPT account/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));

    expect(await within(dialog).findByRole("heading", { name: "Sign in with your ChatGPT account" })).toBeVisible();
    expect(within(dialog).getByRole("textbox", { name: "Name" })).toHaveValue("Other plan");
    expect(within(dialog).queryByRole("textbox", { name: "Address of the page" })).not.toBeInTheDocument();
    await hoverReason(user, within(dialog).getByRole("button", { name: "Add account" }), "Continue with ChatGPT first");
    const second = { ...ATTEMPT, attempt: "019a1b2c-3d4e-7f60-8a9b-0c1d2e3f4a5c" };
    on("POST", `${CREDENTIALS}/oauth`, second);
    await user.click(within(dialog).getByRole("button", { name: "Continue with ChatGPT" }));
    const address = await within(dialog).findByRole("textbox", { name: "Address of the page" });
    expect(address).toHaveValue("");
    await user.type(address, CALLBACK);
    await user.click(within(dialog).getByRole("button", { name: "Add account" }));

    expect(await notifications().findByText("Ada's plan is the OpenAI default")).toBeInTheDocument();
    expect(requests("POST", `${CREDENTIALS}/oauth`).map((r) => r.body)).toEqual([
      { provider: "openai", name: "Plan", make_default: false },
      { provider: "openai", name: "Other plan", make_default: false },
    ]);
    expect(requests("POST", `${CREDENTIALS}/oauth/:attempt`).map((r) => r.params.attempt)).toEqual([second.attempt]);
  });

  it("goes back to an API key when another provider is chosen", async () => {
    const { user, dialog } = await open();
    await choose(user, dialog, "OpenAI");
    await user.click(within(dialog).getByRole("radio", { name: /^ChatGPT account/ }));
    await user.click(within(dialog).getByRole("radio", { name: /^API key/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByRole("heading", { name: "Paste your OpenAI API key" })).toBeVisible();
    await user.click(within(dialog).getByRole("button", { name: "Back" }));
    await user.click(within(dialog).getByRole("radio", { name: /^ChatGPT account/ }));
    await user.click(within(dialog).getByRole("button", { name: "Back" }));
    await choose(user, dialog, "Anthropic");
    expect(await within(dialog).findByRole("heading", { name: "Paste your Anthropic API key" })).toBeVisible();
    await user.click(within(dialog).getByRole("button", { name: "Back" }));
    await choose(user, dialog, "OpenAI");
    expect(within(dialog).getByRole("radio", { name: /^API key/ })).toHaveAttribute("aria-checked", "true");
  });

  it("closes the new tab when the sign-in can't begin and links the page when the browser blocks the tab", async () => {
    const { tab, open: opened } = newTab();
    on("POST", `${CREDENTIALS}/oauth`, apiError(503, "unavailable", "ChatGPT can't be reached right now; try again shortly."));
    const { user, dialog, onClose } = await open();
    await choose(user, dialog, "OpenAI");
    await user.click(within(dialog).getByRole("radio", { name: /^ChatGPT account/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.type(await within(dialog).findByRole("textbox", { name: "Name" }), "Plan");
    await user.click(within(dialog).getByRole("button", { name: "Continue with ChatGPT" }));
    expect((await notifications().findAllByText("ChatGPT can't be reached right now; try again shortly.")).length).toBeGreaterThan(0);
    expect(tab.close).toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();

    opened.mockReturnValue(null);
    on("POST", `${CREDENTIALS}/oauth`, ATTEMPT);
    await user.click(within(dialog).getByRole("button", { name: "Continue with ChatGPT" }));
    expect(await within(dialog).findByRole("link", { name: /Open the ChatGPT sign-in page/ })).toHaveAttribute("href", AUTHORIZE);
  });

  it("signs in again to an account's key", async () => {
    const { tab } = newTab();
    on("POST", `${CREDENTIALS}/:credential/oauth`, ATTEMPT);
    on("POST", `${CREDENTIALS}/:credential/oauth/:attempt`, plan);
    const { user, dialog, onClose } = await open({ keys: [plan], replace: plan });

    expect(within(dialog).getByRole("heading", { name: "Sign in again to OpenAI · Ada's plan" })).toBeInTheDocument();
    expect(within(dialog).getByRole("heading", { name: "Sign in with your ChatGPT account" })).toBeInTheDocument();
    expect(within(dialog).queryByRole("textbox", { name: "Name" })).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Continue with ChatGPT" }));
    await waitFor(() => expect(tab.location.href).toBe(AUTHORIZE));
    expect(requests("POST", `${CREDENTIALS}/:credential/oauth`).map((r) => r.params.credential)).toEqual(["cred_plan"]);
    await user.type(within(dialog).getByRole("textbox", { name: "Address of the page" }), CALLBACK);
    await user.click(within(dialog).getByRole("button", { name: "Sign in again" }));

    expect(await notifications().findByText("OpenAI · Ada's plan signed in again")).toBeInTheDocument();
    expect(requests("POST", `${CREDENTIALS}/:credential/oauth/:attempt`).map((r) => [r.params.credential, r.params.attempt, r.body])).toEqual([
      ["cred_plan", ATTEMPT.attempt, { callback_url: CALLBACK }],
    ]);
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });

  it("shows why an address was refused and keeps the dialog open", async () => {
    newTab();
    on("POST", `${CREDENTIALS}/oauth`, ATTEMPT);
    on(
      "POST",
      `${CREDENTIALS}/oauth/:attempt`,
      apiError(400, "invalid_request", "Paste the address of the page that didn't load; it starts with `http://127.0.0.1`.", { param: "callback_url" }),
    );
    const { user, dialog, onClose } = await open();
    await choose(user, dialog, "OpenAI");
    await user.click(within(dialog).getByRole("radio", { name: /^ChatGPT account/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.type(await within(dialog).findByRole("textbox", { name: "Name" }), "Plan");
    await user.click(within(dialog).getByRole("button", { name: "Continue with ChatGPT" }));
    await user.type(await within(dialog).findByRole("textbox", { name: "Address of the page" }), "https://example.com");
    await user.click(within(dialog).getByRole("button", { name: "Add account" }));
    expect((await notifications().findAllByText(/Paste the address of the page that didn't load; it starts with/)).length).toBeGreaterThan(0);
    expect(onClose).not.toHaveBeenCalled();
  });

  it("signs in to a Grok account with a code, its page and a QR code, checking until it is approved", async () => {
    const opened = vi.spyOn(window, "open");
    const started = grokAttempt();
    on("POST", `${CREDENTIALS}/oauth`, started);
    const later5 = { interval: 5, expires_at: started.expires_at, expires_in: 595 };
    on("POST", `${CREDENTIALS}/oauth/:attempt`, answers(later5, { ...later5, interval: 10 }, { ...later5, interval: 10 }, grokKey));
    const { user, dialog, onClose } = await open();

    await choose(user, dialog, "SpaceX");
    const account = within(dialog).getByRole("radio", { name: /^Grok account/ });
    expect(within(account.closest("label") ?? account).getByText("Experimental")).toBeInTheDocument();
    expect(within(dialog).getByRole("radio", { name: /^API key/ })).not.toHaveTextContent("Experimental");
    await user.click(account);
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByRole("heading", { name: "Sign in with your Grok account" })).toBeVisible();
    expect(within(dialog).getByText("Use your own account only; sharing it breaks SpaceX's terms.")).toBeInTheDocument();
    await hoverReason(user, within(dialog).getByRole("button", { name: "Add account" }), "Give the key a name");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Ada's Grok");
    await hoverReason(user, within(dialog).getByRole("button", { name: "Add account" }), "Get a sign-in code first");
    await getCode(dialog);

    expect(within(dialog).getByText("FKFF-E5EP")).toBeInTheDocument();
    expect(opened).not.toHaveBeenCalled();
    expect(requests("POST", `${CREDENTIALS}/oauth`).map((r) => r.body)).toEqual([{ provider: "xai", name: "Ada's Grok", make_default: false }]);
    expect(within(dialog).getByRole("button", { name: "Copy code" })).toBeInTheDocument();
    const hint = pageHint(dialog, "Open accounts.x.ai on any device, sign in to Grok and enter the code.");
    expect(hint.hint).toContainElement(hint.link);
    expect(hint.link).toHaveAttribute("href", `${GROK_PAGE}?user_code=FKFF-E5EP`);
    expect(within(dialog).getByRole("img", { name: "QR code to open the Grok sign-in page" })).toBeInTheDocument();
    expect(within(dialog).getByRole("status")).toHaveTextContent("Waiting for your approval; the code expires in 10 min.");
    expect(within(dialog).getByText("Kaeso checks every 5 seconds.")).toBeInTheDocument();
    expect(within(dialog).queryByRole("button", { name: "Get a sign-in code" })).not.toBeInTheDocument();

    const polls = () => requests("POST", `${CREDENTIALS}/oauth/:attempt`);
    await later(4000);
    expect(polls()).toHaveLength(0);
    await later(1000);
    expect(polls().map((r) => [r.params.attempt, r.body])).toEqual([[started.attempt, {}]]);
    expect(within(dialog).getByRole("status")).toHaveTextContent("Waiting for your approval; the code expires in 9 min 55 s.");
    await later(5000);
    expect(polls()).toHaveLength(2);
    expect(within(dialog).getByText("Kaeso checks every 10 seconds.")).toBeInTheDocument();
    await later(5000);
    expect(polls()).toHaveLength(2);
    await later(5000);
    expect(polls()).toHaveLength(3);
    await later(10000);
    expect(polls()).toHaveLength(4);
    vi.useRealTimers();

    expect(await notifications().findByText("Ada's Grok is the SpaceX default")).toBeInTheDocument();
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });

  it("ends a declined Grok sign-in and gets a new code", async () => {
    on("POST", `${CREDENTIALS}/oauth`, answers(grokAttempt(), grokAttempt(600, "WXYZ-9876")));
    on("POST", `${CREDENTIALS}/oauth/:attempt`, apiError(400, "invalid_request", "You declined the sign-in at Grok; start again."));
    const { user, dialog, onClose } = await open();
    await choose(user, dialog, "SpaceX");
    await user.click(within(dialog).getByRole("radio", { name: /^Grok account/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.type(await within(dialog).findByRole("textbox", { name: "Name" }), "Grok");
    await getCode(dialog);
    expect(within(dialog).getByText("FKFF-E5EP")).toBeInTheDocument();
    await later(5000);
    vi.useRealTimers();
    expect((await notifications().findAllByText("You declined the sign-in at Grok; start again.")).length).toBeGreaterThan(0);
    expect(await within(dialog).findByRole("status")).toHaveTextContent("This sign-in ended; get a new code to try again.");
    expect(within(dialog).queryByText("Kaeso checks every 5 seconds.")).not.toBeInTheDocument();
    await hoverReason(user, within(dialog).getByRole("button", { name: "Add account" }), "Get a new code first");
    await user.click(within(dialog).getByRole("button", { name: "Get a new code" }));
    expect(await within(dialog).findByText("WXYZ-9876")).toBeInTheDocument();
    expect(within(dialog).getByRole("status")).toHaveTextContent(/^Waiting for your approval/);
    expect(requests("POST", `${CREDENTIALS}/oauth`)).toHaveLength(2);
    expect(onClose).not.toHaveBeenCalled();
  });

  it("stops checking a Grok code once it expires", async () => {
    const started = { ...grokAttempt(12), device: { ...grokAttempt().device, verification_uri_complete: null } };
    on("POST", `${CREDENTIALS}/oauth`, started);
    on(
      "POST",
      `${CREDENTIALS}/oauth/:attempt`,
      answers({ interval: 5, expires_at: started.expires_at, expires_in: 7 }, { interval: 5, expires_at: started.expires_at, expires_in: 2 }),
    );
    const { user, dialog } = await open();
    await choose(user, dialog, "SpaceX");
    await user.click(within(dialog).getByRole("radio", { name: /^Grok account/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.type(await within(dialog).findByRole("textbox", { name: "Name" }), "Grok");
    await getCode(dialog);
    expect(within(dialog).getByText("FKFF-E5EP")).toBeInTheDocument();
    await later(5000);
    await later(5000);
    expect(requests("POST", `${CREDENTIALS}/oauth/:attempt`)).toHaveLength(2);
    expect(within(dialog).getByRole("link", { name: /^accounts\.x\.ai/ })).toHaveAttribute("href", GROK_PAGE);
    await later(3000);
    expect(within(dialog).getByRole("status")).toHaveTextContent("The code expired; get a new code to try again.");
    await later(30000);
    expect(requests("POST", `${CREDENTIALS}/oauth/:attempt`)).toHaveLength(2);
    expect(within(dialog).getByRole("button", { name: "Get a new code" })).toBeInTheDocument();
  });

  async function waitingForGrok(started = grokAttempt()) {
    on("POST", `${CREDENTIALS}/oauth`, started);
    const view = await open();
    await choose(view.user, view.dialog, "SpaceX");
    await view.user.click(within(view.dialog).getByRole("radio", { name: /^Grok account/ }));
    await view.user.click(within(view.dialog).getByRole("button", { name: "Continue" }));
    await view.user.type(await within(view.dialog).findByRole("textbox", { name: "Name" }), "Grok");
    await getCode(view.dialog);
    return view;
  }

  const polled = () => requests("POST", `${CREDENTIALS}/oauth/:attempt`);

  it("counts a Grok code down from the seconds the gateway gave, whatever the clocks say", async () => {
    const { dialog } = await waitingForGrok({ ...grokAttempt(), expires_at: "2020-01-01T00:00:00Z" });
    expect(within(dialog).getByRole("status")).toHaveTextContent("Waiting for your approval; the code expires in 10 min.");
  });

  it("stops checking a Grok code once the method goes back to an API key", async () => {
    const started = grokAttempt();
    on("POST", `${CREDENTIALS}/oauth/:attempt`, { interval: 5, expires_at: started.expires_at, expires_in: 595 });
    const { dialog } = await waitingForGrok(started);
    fireEvent.click(within(dialog).getByRole("button", { name: "Back" }));
    await later(0);
    fireEvent.click(within(dialog).getByRole("radio", { name: /^API key/ }));
    await later(30000);
    expect(polled()).toHaveLength(0);
  });

  it("keeps checking a Grok code through an outage and ends it only when the gateway refuses it", async () => {
    const started = grokAttempt();
    on(
      "POST",
      `${CREDENTIALS}/oauth/:attempt`,
      answers(
        apiError(503, "unavailable", "Grok can't be reached right now; try again shortly."),
        { interval: 5, expires_at: started.expires_at, expires_in: 590 },
        apiError(400, "invalid_request", "You declined the sign-in at Grok; start again."),
      ),
    );
    const { dialog } = await waitingForGrok(started);
    await later(5000);
    expect(polled()).toHaveLength(1);
    expect(within(dialog).getByRole("status")).toHaveTextContent(/^Waiting for your approval/);
    await later(5000);
    expect(polled()).toHaveLength(2);
    await later(5000);
    expect(polled()).toHaveLength(3);
    expect(within(dialog).getByRole("status")).toHaveTextContent("This sign-in ended; get a new code to try again.");
    await later(30000);
    expect(polled()).toHaveLength(3);
  });

  it("drops a Grok check that was still on its way when the name changed", async () => {
    const started = grokAttempt();
    let answer: (reply: unknown) => void = () => undefined;
    on("POST", `${CREDENTIALS}/oauth/:attempt`, () => new Promise((resolve) => (answer = resolve)));
    const { dialog } = await waitingForGrok(started);
    await later(5000);
    expect(polled()).toHaveLength(1);
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Name" }), { target: { value: "Renamed" } });
    answer({ interval: 5, expires_at: started.expires_at, expires_in: 590 });
    await later(0);
    await later(30000);
    expect(polled()).toHaveLength(1);
    expect(within(dialog).queryByText("FKFF-E5EP")).not.toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Get a sign-in code" })).toBeInTheDocument();
  });

  it("signs in again to a Grok key with a new code", async () => {
    const started = grokAttempt();
    on("POST", `${CREDENTIALS}/:credential/oauth`, started);
    on("POST", `${CREDENTIALS}/:credential/oauth/:attempt`, grokKey);
    const { user, dialog, onClose } = await open({ keys: [grokKey], replace: grokKey });
    expect(within(dialog).getByRole("heading", { name: "Sign in again to SpaceX · Ada's Grok" })).toBeInTheDocument();
    expect(within(dialog).queryByRole("textbox", { name: "Name" })).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Get a sign-in code" }));
    expect(await within(dialog).findByText("FKFF-E5EP")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Sign in again" }));
    expect(await notifications().findByText("SpaceX · Ada's Grok signed in again")).toBeInTheDocument();
    expect(requests("POST", `${CREDENTIALS}/:credential/oauth/:attempt`).map((r) => [r.params.credential, r.params.attempt, r.body])).toEqual([
      ["cred_grok", started.attempt, {}],
    ]);
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });

  it("closes on cancel", async () => {
    const { user, dialog, onClose } = await open();
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(onClose).toHaveBeenCalled();
    expect(requests("POST", CREDENTIALS)).toHaveLength(0);
  });
});
