import { describe, expect, it } from "vitest";
import { screen, within } from "@testing-library/react";
import { ORG, PROJECT, signedIn } from "~/test/fixtures";
import { fill, hoverReason, notifications, paged } from "~/test/fixtures/org";
import { on, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import type { GuardrailTestResult, GuardrailView } from "~/api/generated/types";

const PATH = `/settings/orgs/${ORG}/guardrails`;
const LIST = "orgs/:org/guardrails";
const ONE = "orgs/:org/guardrails/:guardrail";
const TRY = "orgs/:org/guardrails/try";

function rules(over: Partial<GuardrailView> = {}): GuardrailView {
  return {
    id: "grd_rules",
    kind: "rules",
    name: "Built-in",
    url: null,
    rules: { detectors: { email: "mask" }, custom: [{ name: "order_id", pattern: "ORD-\\d{6}", action: "flag" }], mask_log: false, restore: false },
    timeout_ms: 2000,
    on_error: "block",
    phases: ["request"],
    streams: "skip",
    projects: [PROJECT],
    created_by: { kind: "user", id: "usr_ada", name: "Ada Okafor" },
    created_at: "2026-11-01T10:00:00Z",
    updated_at: "2026-11-01T10:00:00Z",
    revision: 2,
    ...over,
  } as GuardrailView;
}

const tried = (over: Partial<GuardrailTestResult> = {}): GuardrailTestResult => ({
  action: "replace",
  category: null,
  message: null,
  messages: [{ role: "user", content: [{ type: "text", text: "Mail [EMAIL_1]" }] }],
  failure: null,
  status: null,
  duration_ms: 1,
  found: { email: 1, api_key: 2 },
  ...over,
});

describe("built-in rules", () => {
  it("adds rules with detectors, a pattern and both switches, and tries them on a sample", async () => {
    signedIn();
    on("GET", LIST, paged([]));
    on("POST", LIST, { guardrail: rules(), secret: null });
    let n = 0;
    on("POST", TRY, () => (n++ === 0 ? tried() : tried({ action: "block", message: "it contains an email address.", messages: null, found: {} })));
    const { user } = await renderApp(PATH);
    await screen.findByText("No guardrails yet");
    await user.click(screen.getAllByRole("button", { name: "Add guardrail" })[0] as HTMLElement);
    const dialog = await screen.findByRole("dialog", { name: "Add guardrail" });
    await user.click(within(dialog).getByRole("radio", { name: /Built-in rules/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await hoverReason(user, within(dialog).getByRole("button", { name: "Add guardrail" }), "Give the guardrail a name");
    await fill(user, within(dialog).getByLabelText("Name"), "Built-in");

    await user.click(within(dialog).getByRole("combobox", { name: "Secrets and API keys" }));
    await user.click(await screen.findByRole("option", { name: "Off" }));
    await hoverReason(user, within(dialog).getByRole("button", { name: "Add guardrail" }), "Turn on a detector or add a pattern");
    await user.click(within(dialog).getByRole("combobox", { name: "Email addresses" }));
    await user.click(await screen.findByRole("option", { name: "Mask" }));

    await user.click(within(dialog).getByRole("button", { name: "Add pattern" }));
    await user.click(within(dialog).getByRole("button", { name: "Add pattern" }));
    await hoverReason(user, within(dialog).getByRole("button", { name: "Add guardrail" }), "Give every pattern a name and an expression");
    await fill(user, within(dialog).getAllByLabelText("Pattern name")[0] as HTMLElement, "order_id");
    await fill(user, within(dialog).getAllByLabelText("Regular expression")[0] as HTMLElement, "ORD-\\d{6}");
    await user.click(within(dialog).getByRole("combobox", { name: "What order_id does" }));
    await user.click(await screen.findByRole("option", { name: "Count only" }));
    await user.click(within(dialog).getByRole("button", { name: "Remove pattern 2" }));
    await user.click(within(dialog).getByRole("switch", { name: /Mask in the request log/ }));
    await user.click(within(dialog).getByRole("switch", { name: /Restore in the reply/ }));

    await fill(user, within(dialog).getByLabelText("Sample text"), "Mail ada@example.com");
    await user.click(within(dialog).getByRole("button", { name: "Try rules" }));
    expect(await within(dialog).findByText("Mail [EMAIL_1]")).toBeInTheDocument();
    expect(within(dialog).getByText("1 email address, 2 API keys")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Try rules" }));
    expect(await within(dialog).findByText("it contains an email address.")).toBeInTheDocument();
    expect(within(dialog).getByText("Nothing")).toBeInTheDocument();

    const expected = {
      detectors: { email: "mask" },
      custom: [{ name: "order_id", pattern: "ORD-\\d{6}", action: "flag" }],
      mask_log: true,
      restore: true,
    };
    expect(requests("POST", TRY)[0]?.body).toEqual({ rules: expected, text: "Mail ada@example.com" });
    await user.click(within(dialog).getByRole("button", { name: "Add guardrail" }));
    expect(await notifications().findByText("Guardrail added")).toBeInTheDocument();
    expect(requests("POST", LIST).map((r) => r.body)).toEqual([{ kind: "rules", name: "Built-in", rules: expected }]);
  });

  it("lists rules without a secret to replace, and edits them", async () => {
    signedIn();
    on("GET", LIST, paged([rules()]));
    on("PATCH", ONE, rules({ name: "Rules" }));
    const { user } = await renderApp(PATH);
    const grid = await screen.findByRole("grid", { name: "Guardrails" });
    expect(await within(grid).findByRole("gridcell", { name: /Built-in/ })).toHaveTextContent("Built-in rules");
    await user.click(screen.getByRole("button", { name: "Actions for Built-in" }));
    expect(screen.queryByRole("menuitem", { name: "New secret" })).not.toBeInTheDocument();
    await user.click(await screen.findByRole("menuitem", { name: "Edit" }));
    const dialog = await screen.findByRole("dialog", { name: "Edit Built-in" });
    expect(within(dialog).getByRole("combobox", { name: "What order_id does" })).toHaveTextContent("Count only");
    await user.clear(within(dialog).getByLabelText("Name"));
    await hoverReason(user, within(dialog).getByRole("button", { name: "Save" }), "Give the guardrail a name");
    await fill(user, within(dialog).getByLabelText("Name"), "Rules");
    await user.click(within(dialog).getByRole("switch", { name: /Restore in the reply/ }));
    await user.click(within(dialog).getByRole("button", { name: "Save" }));
    expect(await notifications().findByText("Rules saved")).toBeInTheDocument();
    expect(requests("PATCH", ONE)[0]?.body).toEqual({
      name: "Rules",
      rules: { detectors: { email: "mask" }, custom: [{ name: "order_id", pattern: "ORD-\\d{6}", action: "flag" }], mask_log: false, restore: true },
    });
  });

  it("shows what a test of saved rules found", async () => {
    signedIn();
    on("GET", LIST, paged([rules()]));
    on("POST", `${ONE}/test`, tried({ found: { order_id: 1, private_key: 1, token: 2, phone: 1, iban: 1, card: 1, ip: 3 } }));
    const { user } = await renderApp(PATH);
    await screen.findByRole("grid", { name: "Guardrails" });
    await user.click(screen.getByRole("button", { name: "Actions for Built-in" }));
    await user.click(await screen.findByRole("menuitem", { name: "Test" }));
    const dialog = await screen.findByRole("dialog", { name: "Test Built-in" });
    await fill(user, within(dialog).getByLabelText("Prompt"), "ORD-123456");
    await user.click(within(dialog).getByRole("button", { name: "Send sample" }));
    expect(
      await within(dialog).findByText("1 match of order_id, 1 private key, 2 JSON Web Tokens, 1 phone number, 1 IBAN, 1 card number, 3 IP addresses"),
    ).toBeInTheDocument();
  });
});
