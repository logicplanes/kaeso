import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ORG } from "~/test/fixtures";
import { hoverReason, notifications } from "~/test/fixtures/org";
import { credential } from "~/test/fixtures/shell";
import { apiError, on, requests } from "~/test/network";
import { renderUi } from "~/test/render";
import { ProviderKeyDialog } from "./provider-key-dialog";

const CREDENTIALS = "orgs/:org/credentials";

async function open(props: Partial<Parameters<typeof ProviderKeyDialog>[0]> = {}) {
  const onClose = vi.fn();
  const view = await renderUi(<ProviderKeyDialog org={ORG} keys={[]} onClose={onClose} {...props} />);
  const dialog = await screen.findByRole("dialog", { name: /provider key|Connect|Replace/ });
  return { ...view, dialog, onClose };
}

describe("ProviderKeyDialog", () => {
  it("adds a key for the provider you pick", async () => {
    on("POST", CREDENTIALS, credential({ provider: "anthropic", name: "Messages prod", is_default: false }));
    const { user, dialog, onClose } = await open({ keys: [credential(), credential({ id: "cred_two", is_default: false })] });

    expect(within(dialog).getByRole("heading", { name: "Add provider key" })).toBeInTheDocument();
    expect(
      within(dialog)
        .getByRole("radio", { name: /OpenAI/ })
        .closest("label"),
    ).toHaveTextContent("2 keys");
    expect(
      within(dialog)
        .getByRole("radio", { name: /Anthropic/ })
        .closest("label"),
    ).toHaveTextContent("Not connected yet");
    await user.click(within(dialog).getByRole("radio", { name: /Anthropic/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));

    expect(await within(dialog).findByRole("heading", { name: "Paste your Anthropic API key" })).toBeVisible();
    expect(within(dialog).getByText("Create one at console.anthropic.com under API keys; it starts with sk-ant-.")).toBeInTheDocument();
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

    await user.click(within(dialog).getByRole("radio", { name: /DeepSeek/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));

    expect(await within(dialog).findByRole("heading", { name: "Paste your DeepSeek API key" })).toBeVisible();
    expect(within(dialog).getByText("Create one at platform.deepseek.com under API keys; it starts with sk-.")).toBeInTheDocument();
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

  it("adds an xAI key with xAI's own key hint", async () => {
    on("POST", CREDENTIALS, credential({ provider: "xai", name: "xAI prod", is_default: true }));
    const { user, dialog } = await open();

    await user.click(within(dialog).getByRole("radio", { name: /xAI/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));

    expect(await within(dialog).findByRole("heading", { name: "Paste your xAI API key" })).toBeVisible();
    expect(within(dialog).getByText("Create one at console.x.ai under API keys; it starts with xai-.")).toBeInTheDocument();
    const secret = within(dialog).getByLabelText("xAI API key");
    expect(secret).toHaveAttribute("placeholder", "xai-…");
    await user.type(secret, "xai-123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "xAI prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("xAI prod is the xAI default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([{ provider: "xai", name: "xAI prod", secret: "xai-123", make_default: false }]);
  });

  it("adds a Together AI key", async () => {
    on("POST", CREDENTIALS, credential({ provider: "togetherai", name: "Open models", is_default: true }));
    const { user, dialog } = await open();

    await user.click(within(dialog).getByRole("radio", { name: /Together AI/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));

    expect(await within(dialog).findByRole("heading", { name: "Paste your Together AI API key" })).toBeVisible();
    expect(within(dialog).getByText("Create one at api.together.ai under API keys; it starts with tgp_v1_.")).toBeInTheDocument();
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

    await user.click(within(dialog).getByRole("radio", { name: /Groq/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));

    expect(await within(dialog).findByRole("heading", { name: "Paste your Groq API key" })).toBeVisible();
    expect(within(dialog).getByText("Create one at console.groq.com under API keys; it starts with gsk_.")).toBeInTheDocument();
    const secret = within(dialog).getByLabelText("Groq API key");
    expect(secret).toHaveAttribute("placeholder", "gsk_…");
    await user.type(secret, "gsk_123");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Groq prod");
    await user.click(within(dialog).getByRole("button", { name: "Add key" }));

    expect(await notifications().findByText("Groq prod is the Groq default")).toBeInTheDocument();
    expect(requests("POST", CREDENTIALS).map((r) => r.body)).toEqual([{ provider: "groq", name: "Groq prod", secret: "gsk_123", make_default: false }]);
  });

  it("says what the key step still needs", async () => {
    const { user, dialog } = await open({ provider: "anthropic" });
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

  it("connects a chosen provider and can make the new key its default", async () => {
    on("POST", CREDENTIALS, credential({ name: "Backup", is_default: true }));
    const { user, dialog } = await open({ provider: "openai", keys: [credential()] });

    expect(within(dialog).getByRole("heading", { name: "Connect OpenAI" })).toBeInTheDocument();
    expect(within(dialog).queryByRole("radio", { name: /Anthropic/ })).not.toBeInTheDocument();
    expect(within(dialog).getByText("Create one at platform.openai.com under API keys; it starts with sk-.")).toBeInTheDocument();
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
    const { dialog } = await open({ provider: "mistral" });
    expect(within(dialog).getByRole("heading", { name: "Connect Mistral" })).toBeInTheDocument();
    expect(within(dialog).getByText("Paste the provider's API key.")).toBeInTheDocument();
    expect(within(dialog).getByLabelText("Mistral API key")).toHaveAttribute("placeholder", "sk-…");
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
    const { user, dialog, onClose } = await open({ provider: "openai" });

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
