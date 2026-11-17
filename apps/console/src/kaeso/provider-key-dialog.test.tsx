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

async function open(props: Partial<Parameters<typeof ProviderKeyDialog>[0]> = {}, list = providers) {
  const onClose = vi.fn();
  on("GET", "orgs/:org/providers", page(list));
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
    const { dialog } = await open({ keys: [credential({ id: "cred_acme", provider: "acme", name: "Acme main" })] });
    expect(
      within(dialog)
        .getAllByRole("radio")
        .map((o) => o.textContent),
    ).toHaveLength(providers.length);
    expect(within(dialog).queryByRole("radio", { name: /Acme/ })).not.toBeInTheDocument();
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

  it("adds a Cerebras key with Cerebras's cloud console as its key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "cerebras", name: "Cerebras prod", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "Cerebras");

    expect(await within(dialog).findByRole("heading", { name: "Paste your Cerebras API key" })).toBeVisible();
    expectKeyHint(dialog, "cloud.cerebras.ai", "https://cloud.cerebras.ai", "API Keys", "csk-");
    const secret = within(dialog).getByLabelText("Cerebras API key");
    expect(secret).toHaveAttribute("placeholder", "csk-…");
    await user.type(secret, "csk-123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Cerebras prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Cerebras prod is the Cerebras default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([{ provider: "cerebras", name: "Cerebras prod", secret: "csk-123", make_default: false }]);
  });

  it("adds a MiniMax key with MiniMax's key page as its key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "minimax", name: "MiniMax prod", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "MiniMax");

    expect(await within(dialog).findByRole("heading", { name: "Paste your MiniMax API key" })).toBeVisible();
    expectKeyHint(dialog, "platform.minimax.io", "https://platform.minimax.io/user-center/basic-information/interface-key", "API Keys", "sk-api-");
    const secret = within(dialog).getByLabelText("MiniMax API key");
    expect(secret).toHaveAttribute("placeholder", "sk-api-…");
    await user.type(secret, "sk-api-123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "MiniMax prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("MiniMax prod is the MiniMax default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "minimax", name: "MiniMax prod", secret: "sk-api-123", make_default: false },
    ]);
  });

  it("adds a Fireworks AI key with Fireworks' key page and its fw_ prefix as the key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "fireworks-ai", name: "Fireworks prod", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "Fireworks AI");

    expect(await within(dialog).findByRole("heading", { name: "Paste your Fireworks AI API key" })).toBeVisible();
    expectKeyHint(dialog, "app.fireworks.ai", "https://app.fireworks.ai/settings/users/api-keys", "Settings → API Keys", "fw_");
    const secret = within(dialog).getByLabelText("Fireworks AI API key");
    expect(secret).toHaveAttribute("placeholder", "fw_…");
    await user.type(secret, "fw_123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Fireworks prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Fireworks prod is the Fireworks AI default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "fireworks-ai", name: "Fireworks prod", secret: "fw_123", make_default: false },
    ]);
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
    await user.click(secret);
    await user.paste("AbCdEf0123456789AbCdEf0123456789");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Mistral prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Mistral prod is the Mistral default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "mistral", name: "Mistral prod", secret: "AbCdEf0123456789AbCdEf0123456789", make_default: false },
    ]);
  });

  it("adds a Z.ai key, which has no fixed prefix, with Z.ai's key list as its key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "zai", name: "Z.ai prod", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "Z.ai");

    expect(await within(dialog).findByRole("heading", { name: "Paste your Z.ai API key" })).toBeVisible();
    const link = within(dialog).getByRole("link", { name: "z.ai (opens in a new tab)" });
    expect(link).toHaveAttribute("href", "https://z.ai/manage-apikey/apikey-list");
    expect((link.closest("p") as HTMLElement).textContent?.replace(/⁠? \(opens in a new tab\)/, "")).toBe("Create one at z.ai under API Keys.");
    const secret = within(dialog).getByLabelText("Z.ai API key");
    expect(secret).not.toHaveAttribute("placeholder");
    await user.click(secret);
    await user.paste("0123456789abcdef0123456789abcdef.AbCdEfGhIjKlMnOp");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Z.ai prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Z.ai prod is the Z.ai default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "zai", name: "Z.ai prod", secret: "0123456789abcdef0123456789abcdef.AbCdEfGhIjKlMnOp", make_default: false },
    ]);
  });

  it("adds a Nebius Token Factory key, which has no fixed prefix, with its project's key page as its key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "nebius", name: "Nebius prod", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "Nebius Token Factory");

    expect(await within(dialog).findByRole("heading", { name: "Paste your Nebius Token Factory API key" })).toBeVisible();
    const link = within(dialog).getByRole("link", { name: "tokenfactory.nebius.com (opens in a new tab)" });
    expect(link).toHaveAttribute("href", "https://tokenfactory.nebius.com/project/api-keys");
    expect((link.closest("p") as HTMLElement).textContent?.replace(/⁠? \(opens in a new tab\)/, "")).toBe(
      "Create one at tokenfactory.nebius.com under API keys.",
    );
    const secret = within(dialog).getByLabelText("Nebius Token Factory API key");
    expect(secret).not.toHaveAttribute("placeholder");
    await user.type(secret, "nebius-key-0123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Nebius prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Nebius prod is the Nebius Token Factory default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "nebius", name: "Nebius prod", secret: "nebius-key-0123", make_default: false },
    ]);
  });

  it("adds a SambaNova key, which has no fixed prefix, with SambaCloud's API page as its key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "sambanova", name: "SambaNova prod", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "SambaNova");

    expect(await within(dialog).findByRole("heading", { name: "Paste your SambaNova API key" })).toBeVisible();
    const link = within(dialog).getByRole("link", { name: "cloud.sambanova.ai (opens in a new tab)" });
    expect(link).toHaveAttribute("href", "https://cloud.sambanova.ai/apis");
    expect((link.closest("p") as HTMLElement).textContent?.replace(/⁠? \(opens in a new tab\)/, "")).toBe("Create one at cloud.sambanova.ai under API Keys.");
    const secret = within(dialog).getByLabelText("SambaNova API key");
    expect(secret).not.toHaveAttribute("placeholder");
    await user.type(secret, "sambanova-key-0123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "SambaNova prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("SambaNova prod is the SambaNova default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "sambanova", name: "SambaNova prod", secret: "sambanova-key-0123", make_default: false },
    ]);
  });

  it("adds an Amazon Bedrock key with the region its requests run in", async () => {
    on("POST", CREDENTIALS, credential({ provider: "amazon-bedrock", name: "Bedrock", is_default: true, location: "eu-central-1" }));
    const { user, dialog } = await open();
    const add = () => within(dialog).getByRole("button", { name: "Add key" });

    await choose(user, dialog, "Amazon Bedrock");
    expect(await within(dialog).findByRole("heading", { name: "How does Kaeso sign in to Amazon Bedrock?" })).toBeVisible();
    expect(within(dialog).getByRole("radio", { name: /^Bedrock API key/ })).toBeChecked();
    expect(within(dialog).queryByRole("radio", { name: /^The gateway's AWS identity/ })).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));

    expect(await within(dialog).findByRole("heading", { name: "Paste your Amazon Bedrock API key" })).toBeVisible();
    const link = within(dialog).getByRole("link", { name: "console.aws.amazon.com (opens in a new tab)" });
    expect(link).toHaveAttribute("href", "https://console.aws.amazon.com/bedrock/");
    expect(within(dialog).getByRole("link", { name: "Regions (opens in a new tab)" })).toHaveAttribute(
      "href",
      "https://docs.aws.amazon.com/general/latest/gr/bedrock.html",
    );
    expect(within(dialog).getByLabelText("Amazon Bedrock API key")).not.toHaveAttribute("placeholder");
    await user.type(within(dialog).getByLabelText("Amazon Bedrock API key"), "ABSKexample0123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Bedrock");
    await hoverReason(user, add(), "Choose the key's region");
    await user.click(within(dialog).getByRole("combobox", { name: "Region" }));
    await user.click(await screen.findByRole("option", { name: "eu-central-1" }));
    await user.click(add());

    expect(await notifications().findByText("Bedrock is the Amazon Bedrock default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "amazon-bedrock", name: "Bedrock", secret: "ABSKexample0123", make_default: false, location: "eu-central-1" },
    ]);
  });

  it("adds an Amazon Bedrock key that signs with an AWS access key in its region", async () => {
    on(
      "POST",
      CREDENTIALS,
      credential({
        provider: "amazon-bedrock",
        name: "Signed",
        auth_method: "aws_access_key",
        account: "AKIAIOSFODNN7EXAMPLE",
        location: "eu-central-1",
      }),
    );
    const { user, dialog } = await open();
    const add = () => within(dialog).getByRole("button", { name: "Add key" });

    await choose(user, dialog, "Amazon Bedrock");
    await user.click(await within(dialog).findByRole("radio", { name: /^Access key/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));

    expect(await within(dialog).findByRole("heading", { name: "Add an AWS access key" })).toBeVisible();
    const link = within(dialog).getByRole("link", { name: "console.aws.amazon.com (opens in a new tab)" });
    expect(link).toHaveAttribute("href", "https://console.aws.amazon.com/iam/home#/security_credentials");
    expect((link.closest("p") as HTMLElement).textContent?.replace(/\u2060? \(opens in a new tab\)/, "")).toBe(
      "Create an access key at console.aws.amazon.com under Security credentials.",
    );
    await hoverReason(user, add(), "Give the access key ID");
    await user.type(within(dialog).getByRole("textbox", { name: "Access key ID" }), "AKIAIOSFODNN7EXAMPLE");
    await hoverReason(user, add(), "Paste the secret access key");
    await user.type(within(dialog).getByLabelText("Secret access key"), "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY");
    await user.type(within(dialog).getByLabelText("Session token"), "a b");
    await hoverReason(user, add(), "The session token can't contain spaces");
    await user.clear(within(dialog).getByLabelText("Session token"));
    await user.type(within(dialog).getByLabelText("Session token"), "IQoJb3JpZ2luX2VjEXAMPLE");
    await hoverReason(user, add(), "Choose the key's region");
    await user.click(within(dialog).getByRole("combobox", { name: "Region" }));
    await user.click(await screen.findByRole("option", { name: "eu-central-1" }));
    await hoverReason(user, add(), "Give the key a name");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Signed");
    await user.click(add());

    expect(await notifications().findByText("Signed is the Amazon Bedrock default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      {
        provider: "amazon-bedrock",
        name: "Signed",
        secret: JSON.stringify({
          access_key_id: "AKIAIOSFODNN7EXAMPLE",
          secret_access_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
          session_token: "IQoJb3JpZ2luX2VjEXAMPLE",
        }),
        make_default: false,
        auth_method: "aws_access_key",
        location: "eu-central-1",
      },
    ]);
  });

  it("adds an Amazon Bedrock key that signs in as the gateway's AWS identity when the install lets the admin", async () => {
    const list = providers.map((p) => (p.id === "amazon-bedrock" && p.aws ? { ...p, aws: { ...p.aws, gateway_identity: true } } : p));
    on("POST", CREDENTIALS, credential({ provider: "amazon-bedrock", name: "Role", auth_method: "aws_role", account: "kaeso-bedrock", location: "us-east-1" }));
    const { user, dialog } = await open({}, list);
    const add = () => within(dialog).getByRole("button", { name: "Add key" });

    await choose(user, dialog, "Amazon Bedrock");
    await user.click(await within(dialog).findByRole("radio", { name: /^Access key/ }));
    await user.click(within(dialog).getByRole("radio", { name: /^Bedrock API key/ }));
    await user.click(within(dialog).getByRole("radio", { name: /^The gateway's AWS identity/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));

    expect(await within(dialog).findByRole("heading", { name: "Sign in as the gateway's AWS identity" })).toBeVisible();
    expect(within(dialog).getByText("Kaeso signs each request with the gateway's role, or the role it assumes.")).toBeVisible();
    await user.type(within(dialog).getByRole("textbox", { name: "Role to assume" }), "kaeso-bedrock");
    await hoverReason(user, add(), "Give a role ARN such as arn:aws:iam::…:role/…");
    await user.clear(within(dialog).getByRole("textbox", { name: "Role to assume" }));
    await user.type(within(dialog).getByRole("textbox", { name: "Role to assume" }), "arn:aws:iam::123456789012:role/kaeso-bedrock");
    await user.type(within(dialog).getByRole("textbox", { name: "External ID" }), "a b");
    await hoverReason(user, add(), "An external ID has no spaces");
    await user.clear(within(dialog).getByRole("textbox", { name: "External ID" }));
    await user.type(within(dialog).getByRole("textbox", { name: "External ID" }), "tenant-1");
    await user.click(within(dialog).getByRole("combobox", { name: "Region" }));
    await user.click(await screen.findByRole("option", { name: "us-east-1" }));
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Role");
    await user.click(add());

    await notifications().findByText("Role is the Amazon Bedrock default");
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      {
        provider: "amazon-bedrock",
        name: "Role",
        secret: JSON.stringify({ role_arn: "arn:aws:iam::123456789012:role/kaeso-bedrock", external_id: "tenant-1" }),
        make_default: false,
        auth_method: "aws_role",
        location: "us-east-1",
      },
    ]);
  });

  it("replaces an AWS access key and a gateway identity key with new credentials", async () => {
    const signed = credential({
      id: "cred_aws",
      provider: "amazon-bedrock",
      name: "Signed",
      auth_method: "aws_access_key",
      account: "AKIAIOSFODNN7EXAMPLE",
      location: "eu-central-1",
    });
    on("POST", `${CREDENTIALS}/:credential/replace`, signed);
    const { user, dialog, unmount } = await open({ keys: [signed], replace: signed });

    expect(within(dialog).getByRole("heading", { name: "Add the new access key" })).toBeInTheDocument();
    expect(within(dialog).queryByRole("textbox", { name: "Name" })).not.toBeInTheDocument();
    expect(within(dialog).queryByRole("combobox", { name: "Region" })).not.toBeInTheDocument();
    await user.type(within(dialog).getByRole("textbox", { name: "Access key ID" }), "AKIAI44QH8DHBEXAMPLE");
    await user.type(within(dialog).getByLabelText("Secret access key"), "je7MtGbClwBF/2Zp9Utk/h3yCo8nvbEXAMPLEKEY");
    await user.click(within(dialog).getByRole("button", { name: "Replace key" }));
    await notifications().findByText("Amazon Bedrock · Signed replaced");
    expect(requests("POST", `${CREDENTIALS}/:credential/replace`).map((r) => r.body)).toEqual([
      { secret: JSON.stringify({ access_key_id: "AKIAI44QH8DHBEXAMPLE", secret_access_key: "je7MtGbClwBF/2Zp9Utk/h3yCo8nvbEXAMPLEKEY" }) },
    ]);
    unmount();

    const role = credential({ id: "cred_role", provider: "amazon-bedrock", name: "Role", auth_method: "aws_role", account: "gateway", location: "us-east-1" });
    const again = await open({ keys: [role], replace: role });
    expect(within(again.dialog).getByRole("heading", { name: "Sign in as the gateway's AWS identity" })).toBeInTheDocument();
    await again.user.click(within(again.dialog).getByRole("button", { name: "Replace key" }));
    await waitFor(() => expect(requests("POST", `${CREDENTIALS}/:credential/replace`).at(-1)?.body).toEqual({ secret: "{}" }));
  });

  it("adds a DeepInfra key, which has no fixed prefix, with DeepInfra's dashboard as its key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "deepinfra", name: "DeepInfra prod", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "DeepInfra");

    expect(await within(dialog).findByRole("heading", { name: "Paste your DeepInfra API key" })).toBeVisible();
    const link = within(dialog).getByRole("link", { name: "deepinfra.com (opens in a new tab)" });
    expect(link).toHaveAttribute("href", "https://deepinfra.com/dash/keys");
    expect((link.closest("p") as HTMLElement).textContent?.replace(/⁠? \(opens in a new tab\)/, "")).toBe("Create one at deepinfra.com under API keys.");
    const secret = within(dialog).getByLabelText("DeepInfra API key");
    expect(secret).not.toHaveAttribute("placeholder");
    await user.type(secret, "deepinfra-key-0123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "DeepInfra prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("DeepInfra prod is the DeepInfra default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "deepinfra", name: "DeepInfra prod", secret: "deepinfra-key-0123", make_default: false },
    ]);
  });

  it("adds a Tencent TokenHub key, which has no fixed prefix, with Tencent Cloud's console as its key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "tencent-tokenhub", name: "TokenHub Guangzhou", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "Tencent TokenHub");

    expect(await within(dialog).findByRole("heading", { name: "Paste your Tencent TokenHub API key" })).toBeVisible();
    const link = within(dialog).getByRole("link", { name: "console.cloud.tencent.com (opens in a new tab)" });
    expect(link).toHaveAttribute("href", "https://console.cloud.tencent.com/tokenhub/apikey");
    expect((link.closest("p") as HTMLElement).textContent?.replace(/⁠? \(opens in a new tab\)/, "")).toBe(
      "Create one at console.cloud.tencent.com under API Key management.",
    );
    const secret = within(dialog).getByLabelText("Tencent TokenHub API key");
    expect(secret).not.toHaveAttribute("placeholder");
    await user.type(secret, "tokenhub-key-0123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "TokenHub Guangzhou");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("TokenHub Guangzhou is the Tencent TokenHub default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "tencent-tokenhub", name: "TokenHub Guangzhou", secret: "tokenhub-key-0123", make_default: false },
    ]);
  });

  it("adds a Venice inference key, which has no fixed prefix, with Venice's API settings as its key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "venice", name: "Venice inference", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "Venice");

    expect(await within(dialog).findByRole("heading", { name: "Paste your Venice API key" })).toBeVisible();
    const link = within(dialog).getByRole("link", { name: "venice.ai (opens in a new tab)" });
    expect(link).toHaveAttribute("href", "https://venice.ai/settings/api");
    expect((link.closest("p") as HTMLElement).textContent?.replace(/⁠? \(opens in a new tab\)/, "")).toBe("Create one at venice.ai under API settings.");
    const secret = within(dialog).getByLabelText("Venice API key");
    expect(secret).not.toHaveAttribute("placeholder");
    await user.type(secret, "venice-key-0123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Venice inference");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Venice inference is the Venice default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "venice", name: "Venice inference", secret: "venice-key-0123", make_default: false },
    ]);
  });

  it("adds a Voyage AI model API key, which has no fixed prefix, with MongoDB's key page as its hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "voyage", name: "Voyage retrieval", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "Voyage AI");

    expect(await within(dialog).findByRole("heading", { name: "Paste your Voyage AI API key" })).toBeVisible();
    const link = within(dialog).getByRole("link", { name: "www.mongodb.com (opens in a new tab)" });
    expect(link).toHaveAttribute("href", "https://www.mongodb.com/docs/voyageai/management/api-keys/");
    expect((link.closest("p") as HTMLElement).textContent?.replace(/⁠? \(opens in a new tab\)/, "")).toBe(
      "Create one at www.mongodb.com under Model API keys.",
    );
    const secret = within(dialog).getByLabelText("Voyage AI API key");
    expect(secret).not.toHaveAttribute("placeholder");
    await user.type(secret, "al-voyage-key-0123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Voyage retrieval");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Voyage retrieval is the Voyage AI default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "voyage", name: "Voyage retrieval", secret: "al-voyage-key-0123", make_default: false },
    ]);
  });

  it("adds a Jina AI key with Jina's key manager and its jina_ prefix as the key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "jina", name: "Jina search", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "Jina AI");

    expect(await within(dialog).findByRole("heading", { name: "Paste your Jina AI API key" })).toBeVisible();
    expectKeyHint(dialog, "jina.ai", "https://jina.ai/api-dashboard/key-manager", "API key manager", "jina_");
    const secret = within(dialog).getByLabelText("Jina AI API key");
    expect(secret).toHaveAttribute("placeholder", "jina_…");
    await user.type(secret, "jina_0123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Jina search");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Jina search is the Jina AI default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([{ provider: "jina", name: "Jina search", secret: "jina_0123", make_default: false }]);
  });

  it("adds a StepFun key, which has no fixed prefix, with StepFun's Interface Keys page as its key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "stepfun-ai", name: "StepFun prod", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "StepFun");

    expect(await within(dialog).findByRole("heading", { name: "Paste your StepFun API key" })).toBeVisible();
    const link = within(dialog).getByRole("link", { name: "platform.stepfun.ai (opens in a new tab)" });
    expect(link).toHaveAttribute("href", "https://platform.stepfun.ai/interface-key");
    expect((link.closest("p") as HTMLElement).textContent?.replace(/⁠? \(opens in a new tab\)/, "")).toBe(
      "Create one at platform.stepfun.ai under Interface Keys.",
    );
    const secret = within(dialog).getByLabelText("StepFun API key");
    expect(secret).not.toHaveAttribute("placeholder");
    await user.type(secret, "stepfun-key-0123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "StepFun prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("StepFun prod is the StepFun default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "stepfun-ai", name: "StepFun prod", secret: "stepfun-key-0123", make_default: false },
    ]);
  });

  it("adds an OVHcloud AI Endpoints key, which has no fixed prefix, with the Public Cloud console as its key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "ovhcloud", name: "OVHcloud prod", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "OVHcloud AI Endpoints");

    expect(await within(dialog).findByRole("heading", { name: "Paste your OVHcloud AI Endpoints API key" })).toBeVisible();
    const link = within(dialog).getByRole("link", { name: "www.ovh.com (opens in a new tab)" });
    expect(link).toHaveAttribute("href", "https://www.ovh.com/manager/#/public-cloud/");
    expect((link.closest("p") as HTMLElement).textContent?.replace(/⁠? \(opens in a new tab\)/, "")).toBe(
      "Create one at www.ovh.com under Public Cloud → AI Endpoints → API keys.",
    );
    const secret = within(dialog).getByLabelText("OVHcloud AI Endpoints API key");
    expect(secret).not.toHaveAttribute("placeholder");
    await user.type(secret, "ovhcloud-key-0123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "OVHcloud prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("OVHcloud prod is the OVHcloud AI Endpoints default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "ovhcloud", name: "OVHcloud prod", secret: "ovhcloud-key-0123", make_default: false },
    ]);
  });

  it("adds a Volcengine Ark key, which has no fixed prefix, with Ark's API key page as its key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "volcengine", name: "Ark prod", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "Volcengine Ark");

    expect(await within(dialog).findByRole("heading", { name: "Paste your Volcengine Ark API key" })).toBeVisible();
    const link = within(dialog).getByRole("link", { name: "ark.volcengine.com (opens in a new tab)" });
    expect(link).toHaveAttribute("href", "https://ark.volcengine.com/region:cn-beijing/apiKey");
    expect((link.closest("p") as HTMLElement).textContent?.replace(/⁠? \(opens in a new tab\)/, "")).toBe(
      "Create one at ark.volcengine.com under API Key 管理.",
    );
    const secret = within(dialog).getByLabelText("Volcengine Ark API key");
    expect(secret).not.toHaveAttribute("placeholder");
    await user.click(secret);
    await user.paste("00000000-0000-0000-0000-000000000000");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Ark prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Ark prod is the Volcengine Ark default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "volcengine", name: "Ark prod", secret: "00000000-0000-0000-0000-000000000000", make_default: false },
    ]);
  });

  it("adds an Upstage key, which has no fixed prefix, with Upstage's console as its key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "upstage", name: "Upstage prod", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "Upstage");

    expect(await within(dialog).findByRole("heading", { name: "Paste your Upstage API key" })).toBeVisible();
    const link = within(dialog).getByRole("link", { name: "console.upstage.ai (opens in a new tab)" });
    expect(link).toHaveAttribute("href", "https://console.upstage.ai/api-keys");
    expect((link.closest("p") as HTMLElement).textContent?.replace(/⁠? \(opens in a new tab\)/, "")).toBe("Create one at console.upstage.ai under API keys.");
    const secret = within(dialog).getByLabelText("Upstage API key");
    expect(secret).not.toHaveAttribute("placeholder");
    await user.type(secret, "upstage-key-0123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Upstage prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Upstage prod is the Upstage default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "upstage", name: "Upstage prod", secret: "upstage-key-0123", make_default: false },
    ]);
  });

  it("adds an IONOS token, which has no fixed prefix, with the Data Center Designer's Token Manager as its key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "ionos", name: "IONOS prod", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "IONOS AI Model Hub");

    expect(await within(dialog).findByRole("heading", { name: "Paste your IONOS AI Model Hub API key" })).toBeVisible();
    const link = within(dialog).getByRole("link", { name: "dcd.ionos.com (opens in a new tab)" });
    expect(link).toHaveAttribute("href", "https://dcd.ionos.com");
    expect((link.closest("p") as HTMLElement).textContent?.replace(/⁠? \(opens in a new tab\)/, "")).toBe(
      "Create one at dcd.ionos.com under Management → Token Manager.",
    );
    const secret = within(dialog).getByLabelText("IONOS AI Model Hub API key");
    expect(secret).not.toHaveAttribute("placeholder");
    await user.type(secret, "ionos-token-0123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "IONOS prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("IONOS prod is the IONOS AI Model Hub default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "ionos", name: "IONOS prod", secret: "ionos-token-0123", make_default: false },
    ]);
  });

  it("adds a Standard Compute key, which has no fixed prefix, with its dashboard's API Keys page as its key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "standardcompute", name: "Standard Compute plan", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "Standard Compute");

    expect(await within(dialog).findByRole("heading", { name: "Paste your Standard Compute API key" })).toBeVisible();
    const link = within(dialog).getByRole("link", { name: "standardcompute.com (opens in a new tab)" });
    expect(link).toHaveAttribute("href", "https://standardcompute.com/dashboard/api-keys");
    expect((link.closest("p") as HTMLElement).textContent?.replace(/⁠? \(opens in a new tab\)/, "")).toBe("Create one at standardcompute.com under API Keys.");
    const secret = within(dialog).getByLabelText("Standard Compute API key");
    expect(secret).not.toHaveAttribute("placeholder");
    await user.type(secret, "standardcompute-key-0123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Standard Compute plan");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Standard Compute plan is the Standard Compute default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "standardcompute", name: "Standard Compute plan", secret: "standardcompute-key-0123", make_default: false },
    ]);
  });

  it("adds a Perplexity key with its project's API Keys page and its pplx- prefix as the key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "perplexity", name: "Perplexity search", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "Perplexity");

    expect(await within(dialog).findByRole("heading", { name: "Paste your Perplexity API key" })).toBeVisible();
    expectKeyHint(dialog, "console.perplexity.ai", "https://console.perplexity.ai/project/keys", "API Keys", "pplx-");
    const secret = within(dialog).getByLabelText("Perplexity API key");
    expect(secret).toHaveAttribute("placeholder", "pplx-…");
    await user.type(secret, "pplx-0123abcd");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Perplexity search");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Perplexity search is the Perplexity default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "perplexity", name: "Perplexity search", secret: "pplx-0123abcd", make_default: false },
    ]);
  });

  it("adds a Novita AI key with its Key Management page and its sk_ prefix as the key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "novita-ai", name: "Novita prod", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "Novita AI");

    expect(await within(dialog).findByRole("heading", { name: "Paste your Novita AI API key" })).toBeVisible();
    expectKeyHint(dialog, "novita.ai", "https://novita.ai/settings/key-management", "Key Management", "sk_");
    const secret = within(dialog).getByLabelText("Novita AI API key");
    expect(secret).toHaveAttribute("placeholder", "sk_…");
    await user.type(secret, "sk_0123abcd");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Novita prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Novita prod is the Novita AI default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "novita-ai", name: "Novita prod", secret: "sk_0123abcd", make_default: false },
    ]);
  });

  it("adds a TypeSafe key, which has no fixed prefix, with TypeSafe's console keys page as its key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "typesafe", name: "TypeSafe decisions", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "TypeSafe");

    expect(await within(dialog).findByRole("heading", { name: "Paste your TypeSafe API key" })).toBeVisible();
    const link = within(dialog).getByRole("link", { name: (name) => name.startsWith("console.typesafe.ai") });
    expect(link).toHaveAttribute("href", "https://console.typesafe.ai/keys");
    expect((link.closest("p") as HTMLElement).textContent?.replace(/\u2060? \(opens in a new tab\)/, "")).toBe(
      "Create one at console.typesafe.ai under API keys.",
    );
    const secret = within(dialog).getByLabelText("TypeSafe API key");
    expect(secret).not.toHaveAttribute("placeholder");
    await user.type(secret, "ts-0123abcd");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "TypeSafe decisions");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("TypeSafe decisions is the TypeSafe default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "typesafe", name: "TypeSafe decisions", secret: "ts-0123abcd", make_default: false },
    ]);
  });

  it("adds a Cloudflare AI Gateway token with the account it calls and, optionally, its gateway", async () => {
    on(
      "POST",
      CREDENTIALS,
      credential({
        provider: "cloudflare-ai-gateway",
        name: "Cloudflare",
        is_default: true,
        account_id: "0123456789abcdef0123456789abcdef",
        gateway_id: "prod-eu",
      }),
    );
    const { user, dialog } = await open();
    const add = () => within(dialog).getByRole("button", { name: "Add key" });

    await choose(user, dialog, "Cloudflare AI Gateway");

    expect(await within(dialog).findByRole("heading", { name: "Paste your Cloudflare AI Gateway API key" })).toBeVisible();
    expect(within(dialog).getByRole("link", { name: "Where to find it (opens in a new tab)" })).toHaveAttribute(
      "href",
      "https://developers.cloudflare.com/fundamentals/account/find-account-and-zone-ids/",
    );
    await user.type(within(dialog).getByLabelText("Cloudflare AI Gateway API key"), "cfut_example-token-0123");
    await hoverReason(user, add(), "Give the 32-character account ID");
    const account = within(dialog).getByRole("textbox", { name: "Account ID" });
    await user.type(account, "1234");
    expect(within(dialog).getByText("An account ID is 32 characters, 0–9 and a–f.")).toBeVisible();
    await user.clear(account);
    await user.click(account);
    await user.paste("0123456789ABCDEF0123456789ABCDEF");
    const gateway = within(dialog).getByRole("textbox", { name: "Gateway (optional)" });
    await user.type(gateway, "prod eu");
    await hoverReason(user, add(), "A gateway ID has only letters, digits, hyphens and underscores");
    await user.clear(gateway);
    await user.type(gateway, "prod-eu");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Cloudflare");
    await user.click(add());

    expect(await notifications().findByText("Cloudflare is the Cloudflare AI Gateway default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      {
        provider: "cloudflare-ai-gateway",
        name: "Cloudflare",
        secret: "cfut_example-token-0123",
        make_default: false,
        account_id: "0123456789abcdef0123456789abcdef",
        gateway_id: "prod-eu",
      },
    ]);
  });

  it("adds an Azure key with its resource's endpoint, checked as the gateway checks it", async () => {
    on("POST", CREDENTIALS, credential({ provider: "azure", name: "Azure prod", is_default: true, endpoint: "https://example.openai.azure.com" }));
    const { user, dialog } = await open();
    const add = () => within(dialog).getByRole("button", { name: "Add key" });

    await choose(user, dialog, "Azure");
    expect(await within(dialog).findByRole("heading", { name: "How does Kaeso sign in to Azure?" })).toBeVisible();
    expect(within(dialog).getByRole("radio", { name: /^Resource key/ })).toBeChecked();
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));

    expect(await within(dialog).findByRole("heading", { name: "Paste your Azure API key" })).toBeVisible();
    const link = within(dialog).getByRole("link", { name: "portal.azure.com (opens in a new tab)" });
    expect(link).toHaveAttribute("href", "https://portal.azure.com/");
    expect(within(dialog).getByRole("link", { name: "Where to find it (opens in a new tab)" })).toHaveAttribute(
      "href",
      "https://learn.microsoft.com/en-us/azure/ai-services/authentication",
    );
    // Keys and endpoints are pasted, which also keeps this long test from re-rendering per key.
    await user.click(within(dialog).getByLabelText("Azure API key"));
    await user.paste("0123456789abcdef0123456789abcdef");
    await hoverReason(user, add(), "Paste the endpoint of the key's resource");
    const endpoint = within(dialog).getByRole("textbox", { name: "Endpoint" });
    expect(endpoint).toHaveAttribute("placeholder", "https://example.openai.azure.com");
    for (const [typed, problem] of [
      ["example.openai.azure.com", "Paste the endpoint as a URL, starting with https://."],
      ["http://example.openai.azure.com", "Use an https address without a user name, query or fragment."],
      ["https://example.com", "Use the resource's address, such as https://example.openai.azure.com."],
      ["https://example.openai.azure.com/anthropic", "Leave out the path; Kaeso adds the API's own."],
    ] as const) {
      await user.clear(endpoint);
      await user.paste(typed);
      // The field shows the problem, and the button gives it as its reason.
      expect(endpoint).toHaveAccessibleDescription(expect.stringContaining(problem));
      expect(add()).toHaveAttribute("aria-description", problem);
    }
    await user.clear(endpoint);
    await user.paste("https://example.openai.azure.com/openai/v1/");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Azure prod");
    await user.click(add());

    expect(await notifications().findByText("Azure prod is the Azure default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      {
        provider: "azure",
        name: "Azure prod",
        secret: "0123456789abcdef0123456789abcdef",
        make_default: false,
        endpoint: "https://example.openai.azure.com/openai/v1/",
      },
    ]);
  });

  it("adds an Azure key that signs in with a Microsoft Entra ID service principal", async () => {
    on(
      "POST",
      CREDENTIALS,
      credential({
        provider: "azure",
        name: "Entra",
        auth_method: "azure_service_principal",
        account: "aaaabbbb-0000-cccc-1111-dddd2222eeee",
        endpoint: "https://example.openai.azure.com",
      }),
    );
    const { user, dialog } = await open();
    const add = () => within(dialog).getByRole("button", { name: "Add key" });

    await choose(user, dialog, "Azure");
    await user.click(await within(dialog).findByRole("radio", { name: /^Microsoft Entra ID/ }));
    await user.click(within(dialog).getByRole("radio", { name: /^Resource key/ }));
    await user.click(within(dialog).getByRole("radio", { name: /^Microsoft Entra ID/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));

    expect(await within(dialog).findByRole("heading", { name: "Add a Microsoft Entra ID service principal" })).toBeVisible();
    const link = within(dialog).getByRole("link", { name: "entra.microsoft.com (opens in a new tab)" });
    expect((link.closest("p") as HTMLElement).textContent?.replace(/\u2060? \(opens in a new tab\)/, "")).toBe(
      "Register an app at entra.microsoft.com under App registrations; give it the Cognitive Services OpenAI User role on the resource.",
    );
    await hoverReason(user, add(), "Give the tenant ID or one of its domains");
    await user.click(within(dialog).getByRole("textbox", { name: "Directory (tenant) ID" }));
    await user.paste("contoso.onmicrosoft.com");
    await hoverReason(user, add(), "Give the application (client) ID");
    await user.click(within(dialog).getByRole("textbox", { name: "Application (client) ID" }));
    await user.paste("00001111-aaaa-2222-bbbb-3333cccc4444");
    await hoverReason(user, add(), "Paste the client secret's value");
    await user.click(within(dialog).getByLabelText("Client secret"));
    await user.paste("A1bC2dE3fH4iJ5kL6mN7oP8qR9sT0u");
    await hoverReason(user, add(), "Paste the endpoint of the key's resource");
    await user.click(within(dialog).getByRole("textbox", { name: "Endpoint" }));
    await user.paste("https://example.openai.azure.com");
    await hoverReason(user, add(), "Give the key a name");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Entra");
    await user.click(within(dialog).getByRole("combobox", { name: "Token scope" }));
    await user.click(await screen.findByRole("option", { name: "https://ai.azure.com/.default" }));
    await user.click(add());

    await notifications().findByText("Entra is the Azure default");
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      {
        provider: "azure",
        name: "Entra",
        secret: JSON.stringify({
          tenant_id: "contoso.onmicrosoft.com",
          client_id: "00001111-aaaa-2222-bbbb-3333cccc4444",
          client_secret: "A1bC2dE3fH4iJ5kL6mN7oP8qR9sT0u",
          scope: "https://ai.azure.com/.default",
        }),
        make_default: false,
        auth_method: "azure_service_principal",
        endpoint: "https://example.openai.azure.com",
      },
    ]);
  });

  it("replaces a service principal with new credentials and the first scope", async () => {
    const key = credential({
      id: "cred_entra",
      provider: "azure",
      name: "Entra",
      auth_method: "azure_service_principal",
      account: "aaaabbbb-0000-cccc-1111-dddd2222eeee",
      endpoint: "https://example.openai.azure.com",
      auth_error: "signed_out",
    });
    on("POST", `${CREDENTIALS}/:credential/replace`, { ...key, auth_error: null });
    const { user, dialog } = await open({ keys: [key], replace: key });

    expect(within(dialog).getByRole("heading", { name: "Add the new service principal" })).toBeInTheDocument();
    expect(within(dialog).queryByRole("textbox", { name: "Endpoint" })).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("textbox", { name: "Directory (tenant) ID" }));
    await user.paste("aaaabbbb-0000-cccc-1111-dddd2222eeee");
    await user.click(within(dialog).getByRole("textbox", { name: "Application (client) ID" }));
    await user.paste("00001111-aaaa-2222-bbbb-3333cccc4444");
    await user.click(within(dialog).getByLabelText("Client secret"));
    await user.paste("new-secret");
    await user.click(within(dialog).getByRole("button", { name: "Replace key" }));

    await notifications().findByText("Azure · Entra replaced");
    expect(requests("POST", `${CREDENTIALS}/:credential/replace`).map((r) => r.body)).toEqual([
      {
        secret: JSON.stringify({
          tenant_id: "aaaabbbb-0000-cccc-1111-dddd2222eeee",
          client_id: "00001111-aaaa-2222-bbbb-3333cccc4444",
          client_secret: "new-secret",
          scope: "https://cognitiveservices.azure.com/.default",
        }),
      },
    ]);
  });

  it("adds a Scaleway secret key, which has no fixed prefix, with Scaleway's IAM API keys page as its key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "scaleway", name: "Scaleway prod", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "Scaleway");

    expect(await within(dialog).findByRole("heading", { name: "Paste your Scaleway API key" })).toBeVisible();
    const link = within(dialog).getByRole("link", { name: "console.scaleway.com (opens in a new tab)" });
    expect(link).toHaveAttribute("href", "https://console.scaleway.com/iam/api-keys");
    expect((link.closest("p") as HTMLElement).textContent?.replace(/⁠? \(opens in a new tab\)/, "")).toBe(
      "Create one at console.scaleway.com under IAM → API keys.",
    );
    const secret = within(dialog).getByLabelText("Scaleway API key");
    expect(secret).not.toHaveAttribute("placeholder");
    await user.click(secret);
    await user.paste("00000000-0000-4000-8000-000000000001");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Scaleway prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Scaleway prod is the Scaleway default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "scaleway", name: "Scaleway prod", secret: "00000000-0000-4000-8000-000000000001", make_default: false },
    ]);
  });

  it("adds a Hugging Face token with the access tokens page and its hf_ prefix as the key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "huggingface", name: "Hugging Face prod", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "Hugging Face");

    expect(await within(dialog).findByRole("heading", { name: "Paste your Hugging Face API key" })).toBeVisible();
    expectKeyHint(dialog, "huggingface.co", "https://huggingface.co/settings/tokens", "Settings → Access Tokens", "hf_");
    const secret = within(dialog).getByLabelText("Hugging Face API key");
    expect(secret).toHaveAttribute("placeholder", "hf_…");
    await user.type(secret, "hf_123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Hugging Face prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Hugging Face prod is the Hugging Face default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "huggingface", name: "Hugging Face prod", secret: "hf_123", make_default: false },
    ]);
  });

  it("adds an NVIDIA key with build.nvidia.com's key page and its nvapi- prefix as the key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "nvidia", name: "NVIDIA prod", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "NVIDIA");

    expect(await within(dialog).findByRole("heading", { name: "Paste your NVIDIA API key" })).toBeVisible();
    expectKeyHint(dialog, "build.nvidia.com", "https://build.nvidia.com/settings/api-keys", "API Keys", "nvapi-");
    const secret = within(dialog).getByLabelText("NVIDIA API key");
    expect(secret).toHaveAttribute("placeholder", "nvapi-…");
    await user.type(secret, "nvapi-123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "NVIDIA prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("NVIDIA prod is the NVIDIA default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([{ provider: "nvidia", name: "NVIDIA prod", secret: "nvapi-123", make_default: false }]);
  });

  it("adds a Baseten key, which has no fixed prefix, with Baseten's API keys page as its key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "baseten", name: "Baseten prod", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "Baseten");

    expect(await within(dialog).findByRole("heading", { name: "Paste your Baseten API key" })).toBeVisible();
    const link = within(dialog).getByRole("link", { name: "app.baseten.co (opens in a new tab)" });
    expect(link).toHaveAttribute("href", "https://app.baseten.co/settings/api_keys");
    expect((link.closest("p") as HTMLElement).textContent?.replace(/⁠? \(opens in a new tab\)/, "")).toBe(
      "Create one at app.baseten.co under Organization settings → API keys.",
    );
    const secret = within(dialog).getByLabelText("Baseten API key");
    expect(secret).not.toHaveAttribute("placeholder");
    await user.type(secret, "abcd1234.0123456789");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Baseten prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Baseten prod is the Baseten default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "baseten", name: "Baseten prod", secret: "abcd1234.0123456789", make_default: false },
    ]);
  });

  it("adds a SiliconFlow key with SiliconFlow's API keys page and its sk- prefix as the key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "siliconflow", name: "SiliconFlow prod", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "SiliconFlow");

    expect(await within(dialog).findByRole("heading", { name: "Paste your SiliconFlow API key" })).toBeVisible();
    expectKeyHint(dialog, "cloud.siliconflow.com", "https://cloud.siliconflow.com/account/ak", "API Keys", "sk-");
    const secret = within(dialog).getByLabelText("SiliconFlow API key");
    expect(secret).toHaveAttribute("placeholder", "sk-…");
    await user.type(secret, "sk-123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "SiliconFlow prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("SiliconFlow prod is the SiliconFlow default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "siliconflow", name: "SiliconFlow prod", secret: "sk-123", make_default: false },
    ]);
  });

  it("adds a Google AI Studio key, which has no fixed prefix, with AI Studio's key page as its key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "google", name: "AI Studio prod", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "Google AI Studio");

    expect(await within(dialog).findByRole("heading", { name: "Paste your Google AI Studio API key" })).toBeVisible();
    const link = within(dialog).getByRole("link", { name: "aistudio.google.com (opens in a new tab)" });
    expect(link).toHaveAttribute("href", "https://aistudio.google.com/apikey");
    expect((link.closest("p") as HTMLElement).textContent?.replace(/\u2060? \(opens in a new tab\)/, "")).toBe(
      "Create one at aistudio.google.com under API keys.",
    );
    const secret = within(dialog).getByLabelText("Google AI Studio API key");
    expect(secret).not.toHaveAttribute("placeholder");
    await user.click(secret);
    await user.paste("AIzaSyAbCdEf0123456789AbCdEf0123456789");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "AI Studio prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("AI Studio prod is the Google AI Studio default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "google", name: "AI Studio prod", secret: "AIzaSyAbCdEf0123456789AbCdEf0123456789", make_default: false },
    ]);
  });

  it("adds a Gemini Enterprise Agent Platform service account key from its JSON file with a location", async () => {
    const json = JSON.stringify({
      type: "service_account",
      project_id: "kaeso-example",
      private_key: "-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----\n",
    });
    on(
      "POST",
      CREDENTIALS,
      credential({
        provider: "gemini-agent-platform",
        name: "Agent Platform",
        auth_method: "google_service_account",
        account: "kaeso@kaeso-example.iam.gserviceaccount.com",
        project: "kaeso-example",
        location: "us-east5",
      }),
    );
    const { user, dialog } = await open();

    await choose(user, dialog, "Gemini Enterprise Agent Platform");
    expect(await within(dialog).findByRole("heading", { name: "Add an API key or a service account key?" })).toBeVisible();
    expect(within(dialog).getByRole("radio", { name: /^Service account key/ })).toBeChecked();
    expect(within(dialog).getByRole("radio", { name: /^Express mode API key/ })).toHaveAccessibleDescription("Reaches Gemini models only.");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));

    expect(await within(dialog).findByRole("heading", { name: "Add a service account key" })).toBeVisible();
    const link = within(dialog).getByRole("link", { name: "console.cloud.google.com (opens in a new tab)" });
    expect(link).toHaveAttribute("href", "https://console.cloud.google.com/iam-admin/serviceaccounts");
    expect((link.closest("p") as HTMLElement).textContent?.replace(/\u2060? \(opens in a new tab\)/, "")).toBe(
      "Create a JSON key at console.cloud.google.com under IAM & Admin → Service accounts.",
    );
    await hoverReason(user, within(dialog).getByRole("button", { name: "Add key" }), "Paste or choose the JSON key");
    const file = dialog.querySelector('input[type="file"]') as HTMLInputElement;
    await user.upload(file, new File([json], "kaeso-example-0123.json", { type: "application/json" }));
    await waitFor(() => expect(within(dialog).getByLabelText("JSON key")).toHaveValue(json));
    await user.click(within(dialog).getByRole("combobox", { name: "Location" }));
    await user.click(await screen.findByRole("option", { name: "us-east5" }));
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Agent Platform");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Agent Platform is the Gemini Enterprise Agent Platform default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      {
        provider: "gemini-agent-platform",
        name: "Agent Platform",
        secret: json,
        make_default: false,
        auth_method: "google_service_account",
        location: "us-east5",
      },
    ]);
  });

  it("chooses the service account's key file from the button and refuses a file too large to be one", async () => {
    const { user, dialog } = await open();

    await choose(user, dialog, "Gemini Enterprise Agent Platform");
    await user.click(await within(dialog).findByRole("radio", { name: /^Express mode API key/ }));
    await user.click(within(dialog).getByRole("radio", { name: /^Service account key/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    const file = (await within(dialog).findByText("Choose file")).closest("div")?.querySelector('input[type="file"]') as HTMLInputElement;
    const click = vi.spyOn(file, "click");
    await user.click(within(dialog).getByRole("button", { name: "Choose file" }));
    expect(click).toHaveBeenCalled();
    fireEvent.change(file, { target: { files: [] } });
    await user.upload(file, new File(["x".repeat(16 * 1024 + 1)], "large.json", { type: "application/json" }));
    expect(await notifications().findByText("That file is too large to be a JSON key.")).toBeInTheDocument();
    expect(within(dialog).getByLabelText("JSON key")).toHaveValue("");
  });

  it("adds a Gemini Enterprise Agent Platform express mode key without a location", async () => {
    on("POST", CREDENTIALS, credential({ provider: "gemini-agent-platform", name: "Express", is_default: true }));
    const { user, dialog } = await open();

    await choose(user, dialog, "Gemini Enterprise Agent Platform");
    await user.click(await within(dialog).findByRole("radio", { name: /^Express mode API key/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByRole("heading", { name: "Paste your Gemini Enterprise Agent Platform API key" })).toBeVisible();
    expect(within(dialog).queryByRole("combobox", { name: "Location" })).not.toBeInTheDocument();
    await user.type(within(dialog).getByLabelText("Gemini Enterprise Agent Platform API key"), "AQ.Ab0123456789AbCdEf");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Express");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    await notifications().findByText("Express is the Gemini Enterprise Agent Platform default");
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "gemini-agent-platform", name: "Express", secret: "AQ.Ab0123456789AbCdEf", make_default: false },
    ]);
  });

  it("replaces a service account key with a new JSON key and keeps its location", async () => {
    const key = credential({
      id: "cred_sa",
      provider: "gemini-agent-platform",
      name: "Agent Platform",
      auth_method: "google_service_account",
      account: "kaeso@kaeso-example.iam.gserviceaccount.com",
      project: "kaeso-example",
      location: "eu",
      last4: "cafe",
    });
    on("POST", `${CREDENTIALS}/:credential/replace`, key);
    const { user, dialog } = await open({ keys: [key], replace: key });

    expect(within(dialog).getByRole("heading", { name: "Add the new JSON key" })).toBeInTheDocument();
    expect(within(dialog).queryByRole("textbox", { name: "Name" })).not.toBeInTheDocument();
    await hoverReason(user, within(dialog).getByRole("button", { name: "Replace key" }), "Paste or choose the JSON key");
    fireEvent.change(within(dialog).getByLabelText("JSON key"), { target: { value: "not json" } });
    await hoverReason(user, within(dialog).getByRole("button", { name: "Replace key" }), "Paste the whole JSON key file");
    fireEvent.change(within(dialog).getByLabelText("JSON key"), { target: { value: '{"type":"service_account"}' } });
    await user.click(within(dialog).getByRole("button", { name: "Replace key" }));

    await notifications().findByText("Gemini Enterprise Agent Platform · Agent Platform replaced");
    expect(requests("POST", `${CREDENTIALS}/:credential/replace`).map((r) => r.body)).toEqual([{ secret: '{"type":"service_account"}', location: "eu" }]);
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
    await user.click(secret);
    await user.paste("AbCdEf0123456789AbCdEf0123456789AbCdEf01");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Cohere prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Cohere prod is the Cohere default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "cohere", name: "Cohere prod", secret: "AbCdEf0123456789AbCdEf0123456789AbCdEf01", make_default: false },
    ]);
  });

  it("asks only for the provider's key for a provider it has no hint for", async () => {
    const acme = credential({ id: "cred_acme", provider: "acme", name: "Acme main" });
    const { dialog } = await open({ keys: [acme], replace: acme });
    expect(within(dialog).getByRole("heading", { name: "Paste the new acme key" })).toBeInTheDocument();
    expect(within(dialog).getByText("Paste the provider's API key.")).toBeInTheDocument();
    expect(within(dialog).getByLabelText("acme API key")).not.toHaveAttribute("placeholder");
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
    await user.click(await within(dialog).findByRole("textbox", { name: "Address of the page" }));
    await user.paste(CALLBACK);
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
    await user.click(await within(dialog).findByRole("textbox", { name: "Address of the page" }));
    await user.paste(CALLBACK);
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
    await user.click(await within(dialog).findByRole("textbox", { name: "Address of the page" }));
    await user.paste(CALLBACK);
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
    // The address is pasted, as a person copies it from the other tab.
    await user.click(await within(dialog).findByRole("textbox", { name: "Address of the page" }));
    await user.paste(CALLBACK);

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
    await user.click(address);
    await user.paste(CALLBACK);
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
    await user.click(within(dialog).getByRole("textbox", { name: "Address of the page" }));
    await user.paste(CALLBACK);
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
