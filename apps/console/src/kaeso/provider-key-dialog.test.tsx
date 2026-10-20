import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ORG } from "~/test/fixtures";
import { hoverReason, notifications } from "~/test/fixtures/org";
import { credential } from "~/test/fixtures/shell";
import { apiError, on, requests } from "~/test/network";
import { renderUi } from "~/test/render";
import { ProviderKeyDialog } from "./provider-key-dialog";

const CREDENTIALS = "orgs/:org/credentials";

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
    expect(within(dialog).getByRole("option", { name: "OpenAI 2 keys" })).toHaveAttribute("aria-selected", "true");
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

    expect(await notifications().findByText("Messages prod added")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([
      { provider: "anthropic", name: "Messages prod", secret: "sk-ant-123", make_default: false },
    ]);
    await waitFor(() => expect(onClose).toHaveBeenCalled());
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
    expectKeyHint(dialog, "console.x.ai", "https://console.x.ai", "API Keys", "xai-");
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
    expect(within(dialog).getByText("Step 2 of 3")).toBeInTheDocument();
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

  it("asks only for the provider's key for a provider it has no hint for", async () => {
    const cohere = credential({ id: "cred_cohere", provider: "cohere", name: "Cohere main" });
    const { dialog } = await open({ keys: [cohere], replace: cohere });
    expect(within(dialog).getByRole("heading", { name: "Paste the new Cohere key" })).toBeInTheDocument();
    expect(within(dialog).getByText("Paste the provider's API key.")).toBeInTheDocument();
    expect(within(dialog).getByLabelText("Cohere API key")).toHaveAttribute("placeholder", "sk-…");
  });

  it("replaces a key's secret", async () => {
    on("POST", `${CREDENTIALS}/:credential/replace`, credential());
    const { user, dialog, onClose } = await open({ keys: [credential()], replace: credential() });

    expect(within(dialog).getByRole("heading", { name: "Replace Production" })).toBeInTheDocument();
    expect(within(dialog).getByRole("heading", { name: "Paste the new OpenAI key" })).toBeInTheDocument();
    expect(within(dialog).queryByRole("textbox", { name: "Name" })).not.toBeInTheDocument();
    await user.type(within(dialog).getByLabelText("OpenAI API key"), "sk-rotated");
    await user.keyboard("{Meta>}{Enter}{/Meta}");

    expect(await notifications().findByText("Production replaced")).toBeInTheDocument();
    expect(requests("POST", `${CREDENTIALS}/:credential/replace`).map((r) => [r.params.credential, r.body])).toEqual([["cred_prod", { secret: "sk-rotated" }]]);
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });

  it("shows the API's message and keeps the dialog open when the key is refused", async () => {
    on("POST", CREDENTIALS, apiError(422, "invalid_request", "OpenAI rejected this key.", { param: "secret" }));
    const { user, dialog, onClose } = await open();

    await choose(user, dialog, "OpenAI");
    await user.type(within(dialog).getByLabelText("OpenAI API key"), "sk-bad");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Bad{Enter}");
    expect((await notifications().findAllByText("OpenAI rejected this key.")).length).toBeGreaterThan(0);
    expect(onClose).not.toHaveBeenCalled();
  });

  it("closes on cancel", async () => {
    const { user, dialog, onClose } = await open();
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(onClose).toHaveBeenCalled();
    expect(requests("POST", CREDENTIALS)).toHaveLength(0);
  });
});
