import { screen, waitFor, within } from "@testing-library/react";
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

afterEach(() => vi.restoreAllMocks());

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
  await user.click(within(dialog).getByRole("option", { name: (name) => name.startsWith(provider) }));
  await user.click(within(dialog).getByRole("button", { name: "Continue" }));
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
    expect(within(dialog).getByRole("combobox", { name: "Search providers" })).toBeInTheDocument();
    expect(within(within(dialog).getByRole("group", { name: "Connected" })).getByRole("option")).toHaveAccessibleName("OpenAI 2 keys");
    expect(within(dialog).getByRole("option", { name: "OpenAI 2 keys" })).toHaveAttribute("aria-selected", "false");
    await hoverReason(user, within(dialog).getByRole("button", { name: "Continue" }), "Choose a provider");
    expect(within(dialog).getByRole("group", { name: "Other providers" })).toContainElement(within(dialog).getByRole("option", { name: "Anthropic" }));
    await user.click(within(dialog).getByRole("option", { name: "Anthropic" }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));

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
    const { dialog } = await open({ keys: [credential({ id: "cred_cohere", provider: "cohere", name: "Cohere main" })] });
    expect(
      within(dialog)
        .getAllByRole("option")
        .map((o) => o.textContent),
    ).toHaveLength(providers.length);
    expect(within(dialog).queryByRole("option", { name: /Cohere/ })).not.toBeInTheDocument();
  });

  it("says when the providers can't load and loads them again", async () => {
    let calls = 0;
    on("GET", "orgs/:org/providers", () => (++calls === 1 ? apiError(429, "rate_limited", "Too many requests; try again in a minute.") : page(providers)));
    const onClose = vi.fn();
    const { user } = await renderUi(<ProviderKeyDialog org={ORG} keys={[]} onClose={onClose} />);
    const dialog = await screen.findByRole("dialog", { name: /provider key/ });
    expect(within(dialog).queryByRole("option")).not.toBeInTheDocument();
    expect(await notifications().findByText("Too many requests; try again in a minute.")).toBeInTheDocument();
    await user.click(notifications().getByRole("button", { name: "Try again", hidden: true }));
    expect(await within(dialog).findAllByRole("option")).toHaveLength(providers.length);
  });

  it("adds a DeepSeek key", async () => {
    on("POST", CREDENTIALS, credential({ provider: "deepseek", name: "Reasoning", is_default: true }));
    const { user, dialog } = await open();

    await user.click(within(dialog).getByRole("option", { name: "DeepSeek" }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));

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
    expect(secret).toHaveAttribute("placeholder", "…");
    await user.type(secret, "AbCdEf0123456789AbCdEf0123456789");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Mistral prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Mistral prod is the Mistral default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "mistral", name: "Mistral prod", secret: "AbCdEf0123456789AbCdEf0123456789", make_default: false },
    ]);
  });

  it("asks only for the provider's key for a provider it has no hint for", async () => {
    const cohere = credential({ id: "cred_cohere", provider: "cohere", name: "Cohere main" });
    const { dialog } = await open({ keys: [cohere], replace: cohere });
    expect(within(dialog).getByRole("heading", { name: "Paste the new Cohere key" })).toBeInTheDocument();
    expect(within(dialog).getByText("Paste the provider's API key.")).toBeInTheDocument();
    expect(within(dialog).getByLabelText("Cohere API key")).toHaveAttribute("placeholder", "sk-…");
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

  it("closes on cancel", async () => {
    const { user, dialog, onClose } = await open();
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(onClose).toHaveBeenCalled();
    expect(requests("POST", CREDENTIALS)).toHaveLength(0);
  });
});
