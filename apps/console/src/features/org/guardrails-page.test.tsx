import { describe, expect, it } from "vitest";
import { screen, within } from "@testing-library/react";
import { ADMIN_ORG_RIGHTS, ORG, PROJECT, asAdmin, signedIn } from "~/test/fixtures";
import { fill, hoverReason, notifications, paged } from "~/test/fixtures/org";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import type { GuardrailTestResult, GuardrailView } from "~/api/generated/types";

const PATH = `/settings/orgs/${ORG}/guardrails`;
const LIST = "orgs/:org/guardrails";
const ONE = "orgs/:org/guardrails/:guardrail";

function guardrail(over: Partial<GuardrailView> = {}): GuardrailView {
  return {
    id: "grd_pii",
    kind: "http",
    name: "PII check",
    url: "https://guardrail.example.com/check",
    rules: null,
    timeout_ms: 2000,
    on_error: "block",
    phases: ["request"],
    streams: "skip",
    projects: [PROJECT],
    created_by: { kind: "user", id: "usr_ada", name: "Ada Okafor" },
    created_at: "2026-11-01T10:00:00Z",
    updated_at: "2026-11-01T10:00:00Z",
    revision: 3,
    ...over,
  } as GuardrailView;
}

async function retype(user: Awaited<ReturnType<typeof renderApp>>["user"], field: HTMLElement, text: string) {
  await user.clear(field);
  if (text) await fill(user, field, text);
}

async function rowOf(name: string) {
  const grid = await screen.findByRole("grid", { name: "Guardrails" });
  return within(grid).findByRole("gridcell", { name: new RegExp(name) });
}

async function menuOf(user: Awaited<ReturnType<typeof renderApp>>["user"], name: string) {
  await rowOf(name);
  await user.click(screen.getByRole("button", { name: `Actions for ${name}` }));
}

describe("guardrails", () => {
  it("lists the guardrails with what they check and how many projects use them", async () => {
    signedIn();
    on(
      "GET",
      LIST,
      paged([
        guardrail(),
        guardrail({ id: "grd_out", name: "Output check", phases: ["request", "reply"], streams: "hold", projects: [] }),
        guardrail({ id: "grd_r", name: "Replies only", phases: ["reply"], streams: "skip", projects: [PROJECT, "proj_b"] }),
        guardrail({ id: "grd_both", name: "Both", phases: ["request", "reply"], streams: "skip" }),
        guardrail({ id: "grd_rh", name: "Held replies", phases: ["reply"], streams: "hold" }),
      ]),
    );
    await renderApp(PATH);
    expect(await screen.findByRole("heading", { name: "Guardrails", level: 1 })).toBeInTheDocument();
    const pii = await rowOf("PII check");
    expect(pii).toHaveTextContent("https://guardrail.example.com/check");
    expect(pii).toHaveTextContent("Requests");
    expect(pii).toHaveTextContent("1 project");
    expect(await rowOf("Output check")).toHaveTextContent("Requests and replies, streams held");
    expect(await rowOf("Output check")).toHaveTextContent("No projects");
    expect(await rowOf("Replies only")).toHaveTextContent("Replies");
    expect(await rowOf("Replies only")).toHaveTextContent("2 projects");
    expect(await rowOf("Both")).toHaveTextContent("Requests and replies");
    expect(await rowOf("Held replies")).toHaveTextContent("Replies, streams held");
  });

  it("says when there are none, and when they couldn't load", async () => {
    signedIn();
    on("GET", LIST, paged([]));
    await renderApp(PATH);
    expect(await screen.findByText("No guardrails yet")).toBeInTheDocument();
  });

  it("says when the list couldn't load", async () => {
    signedIn();
    on("GET", LIST, apiError(500, "internal", "Boom."));
    await renderApp(PATH);
    expect(await screen.findByText("The guardrails couldn't load")).toBeInTheDocument();
  });

  it("hides the page from people without the right", async () => {
    asAdmin(ADMIN_ORG_RIGHTS.filter((r) => !r.startsWith("guardrails.")));
    await renderApp(PATH);
    expect(await screen.findByText("You can't see the guardrails here")).toBeInTheDocument();
    expect(requests("GET", LIST)).toHaveLength(0);
  });

  it("adds a guardrail in two steps and shows its secret once", async () => {
    signedIn();
    on("GET", LIST, paged([]));
    on("POST", LIST, { guardrail: guardrail({ phases: ["request", "reply"], streams: "hold", timeout_ms: 500, on_error: "allow" }), secret: "whsec_abc123" });
    const { user } = await renderApp(PATH);
    await screen.findByText("No guardrails yet");
    await user.keyboard("c");
    const dialog = await screen.findByRole("dialog", { name: "Add guardrail" });
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await retype(user, within(dialog).getByLabelText("Name"), "PII check");
    await fill(user, within(dialog).getByLabelText("Address"), "https://guardrail.example.com/check");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(await within(dialog).findByRole("checkbox", { name: /Replies/ }));
    await user.click(within(dialog).getByRole("radio", { name: /Hold and check/ }));
    await retype(user, within(dialog).getByLabelText("Timeout"), "50");
    expect(within(dialog).getByText("Choose 100 to 10000 milliseconds")).toBeInTheDocument();
    await retype(user, within(dialog).getByLabelText("Timeout"), "500");
    await user.click(within(dialog).getByRole("radio", { name: /Let it through/ }));
    await user.click(within(dialog).getByRole("button", { name: "Add guardrail" }));
    expect(await screen.findByRole("textbox", { name: "Key for PII check" })).toHaveTextContent("whsec_abc123");
    expect(requests("POST", LIST).map((r) => r.body)).toEqual([
      {
        kind: "http",
        name: "PII check",
        url: "https://guardrail.example.com/check",
        phases: ["request", "reply"],
        streams: "hold",
        timeout_ms: 500,
        on_error: "allow",
      },
    ]);
    await user.keyboard("{Escape}");
    const confirm = await screen.findByRole("alertdialog", { name: "Did you copy the secret?" });
    await user.click(within(confirm).getByRole("button", { name: "Close without copying" }));
    expect(screen.queryByRole("dialog", { name: "Add guardrail" })).not.toBeInTheDocument();
  });

  it("asks for a name, an address and what to check before it adds one", async () => {
    signedIn();
    on("GET", LIST, paged([]));
    const { user } = await renderApp(PATH);
    await screen.findByText("No guardrails yet");
    await user.click(screen.getAllByRole("button", { name: "Add guardrail" })[0] as HTMLElement);
    const dialog = await screen.findByRole("dialog", { name: "Add guardrail" });
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await hoverReason(user, within(dialog).getByRole("button", { name: "Continue" }), "Give the guardrail a name");
    await retype(user, within(dialog).getByLabelText("Name"), "PII check");
    await hoverReason(user, within(dialog).getByRole("button", { name: "Continue" }), "Enter its address");
    await fill(user, within(dialog).getByLabelText("Address"), "https://guardrail.example.com/check");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(await within(dialog).findByRole("checkbox", { name: /Requests/ }));
    await hoverReason(user, within(dialog).getByRole("button", { name: "Add guardrail" }), "Choose what it checks");
    await user.keyboard("{Escape}");
    expect(requests("POST", LIST)).toHaveLength(0);
  });

  it("edits a guardrail with its revision", async () => {
    signedIn();
    on("GET", LIST, paged([guardrail({ phases: ["reply"] })]));
    on("PATCH", ONE, guardrail({ name: "PII", phases: ["reply"] }));
    const { user } = await renderApp(PATH);
    await menuOf(user, "PII check");
    await user.click(await screen.findByRole("menuitem", { name: "Edit" }));
    const dialog = await screen.findByRole("dialog", { name: "Edit PII check" });
    await user.click(within(dialog).getByRole("radio", { name: /Stream unchecked/ }));
    await retype(user, within(dialog).getByLabelText("Name"), "");
    await hoverReason(user, within(dialog).getByRole("button", { name: "Save" }), "Enter a name and an address");
    await retype(user, within(dialog).getByLabelText("Name"), "PII");
    await user.click(within(dialog).getByRole("checkbox", { name: /Replies/ }));
    await hoverReason(user, within(dialog).getByRole("button", { name: "Save" }), "Choose what it checks");
    await user.click(within(dialog).getByRole("checkbox", { name: /Replies/ }));
    await retype(user, within(dialog).getByLabelText("Timeout"), "20000");
    expect(within(dialog).getByText("Choose 100 to 10000 milliseconds")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Save" })).toHaveAttribute("aria-disabled", "true");
    await retype(user, within(dialog).getByLabelText("Timeout"), "2000");
    await user.click(within(dialog).getByRole("radio", { name: /Refuse the request/ }));
    await user.click(within(dialog).getByLabelText("Address"));
    await user.keyboard("{Enter}");
    expect(await notifications().findByText("PII saved")).toBeInTheDocument();
    const [patch] = requests("PATCH", ONE);
    expect(patch?.body).toEqual({
      name: "PII",
      url: "https://guardrail.example.com/check",
      phases: ["reply"],
      streams: "skip",
      timeout_ms: 2000,
      on_error: "block",
    });
    expect(patch?.headers.get("if-match")).toBe('"3"');
  });

  it("gives a guardrail a new secret", async () => {
    signedIn();
    on("GET", LIST, paged([guardrail()]));
    on("POST", `${ONE}/secret`, { guardrail: guardrail(), secret: "whsec_new" });
    const { user } = await renderApp(PATH);
    await menuOf(user, "PII check");
    await user.click(await screen.findByRole("menuitem", { name: "New secret" }));
    const dialog = await screen.findByRole("dialog", { name: "New signing secret" });
    expect(within(dialog).getByRole("textbox", { name: "Key for PII check" })).toHaveTextContent("whsec_new");
    await user.click(within(dialog).getByRole("button", { name: "Copy key" }));
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "New signing secret" })).not.toBeInTheDocument();
  });

  it("deletes a guardrail after asking", async () => {
    signedIn();
    on("GET", LIST, paged([guardrail()]));
    on("PATCH", ONE, guardrail());
    on("DELETE", ONE, null);
    const { user } = await renderApp(PATH);
    await menuOf(user, "PII check");
    await user.click(await screen.findByRole("menuitem", { name: "Edit" }));
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "Edit PII check" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Actions for PII check" }));
    await user.click(await screen.findByRole("menuitem", { name: "Edit" }));
    await user.click(within(await screen.findByRole("dialog", { name: "Edit PII check" })).getByRole("button", { name: "Save" }));
    expect(await notifications().findByText("PII check saved")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Actions for PII check" }));
    await user.click(await screen.findByRole("menuitem", { name: "Delete" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Delete PII check?" });
    await user.click(within(confirm).getByRole("button", { name: "Delete guardrail" }));
    expect(await notifications().findByText("PII check deleted")).toBeInTheDocument();
    expect(requests("DELETE", ONE)).toHaveLength(1);
  });

  it("closes the delete dialog when the delete fails", async () => {
    signedIn();
    on("GET", LIST, paged([guardrail()]));
    on("DELETE", ONE, apiError(404, "not_found", "No such guardrail, or you cannot see it."));
    const { user } = await renderApp(PATH);
    await menuOf(user, "PII check");
    await user.click(await screen.findByRole("menuitem", { name: "Delete" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Delete PII check?" });
    await user.click(within(confirm).getByRole("button", { name: "Delete guardrail" }));
    expect(await notifications().findByText("No such guardrail, or you cannot see it.")).toBeInTheDocument();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  it("tests a guardrail on a sample and shows every part of the verdict", async () => {
    signedIn();
    on("GET", LIST, paged([guardrail({ phases: ["request", "reply"] })]));
    const answers: GuardrailTestResult[] = [
      { action: "block", category: "pii", message: "No emails.", messages: null, failure: null, status: null, duration_ms: 12, found: {} },
      {
        action: "replace",
        category: null,
        message: null,
        messages: [{ role: "assistant", content: [{ type: "text", text: "[EMAIL]" }, { type: "reasoning" }] }],
        failure: null,
        status: null,
        duration_ms: 9,
        found: {},
      },
      { action: null, category: null, message: null, messages: null, failure: "status", status: 500, duration_ms: 3, found: {} },
    ];
    let n = 0;
    on("POST", `${ONE}/test`, () => answers[n++]);
    const { user } = await renderApp(PATH);
    await menuOf(user, "PII check");
    await user.click(await screen.findByRole("menuitem", { name: "Test" }));
    const dialog = await screen.findByRole("dialog", { name: "Test PII check" });
    await hoverReason(user, within(dialog).getByRole("button", { name: "Send sample" }), "Enter a sample to check");
    await fill(user, within(dialog).getByLabelText("Prompt"), "Mail ada@example.com");
    await user.click(within(dialog).getByRole("button", { name: "Send sample" }));
    expect(await within(dialog).findByText("Blocked")).toBeInTheDocument();
    expect(within(dialog).getByText("pii")).toBeInTheDocument();
    expect(within(dialog).getByText("No emails.")).toBeInTheDocument();
    expect(within(dialog).getByText("12 ms")).toBeInTheDocument();

    await user.click(within(dialog).getByRole("radio", { name: "Replies" }));
    await hoverReason(user, within(dialog).getByRole("button", { name: "Send sample" }), "Enter a sample to check");
    await fill(user, within(dialog).getByLabelText("Reply"), "ada@example.com");
    await user.click(within(dialog).getByRole("button", { name: "Send sample" }));
    expect(await within(dialog).findByText("Rewritten")).toBeInTheDocument();
    expect(within(dialog).getByText("[EMAIL]")).toBeInTheDocument();

    await user.click(within(dialog).getByRole("button", { name: "Send sample" }));
    expect(await within(dialog).findByText("Answered with an error (500)")).toBeInTheDocument();
    expect(within(dialog).getByText("No verdict")).toBeInTheDocument();
    expect(requests("POST", `${ONE}/test`).map((r) => r.body)).toEqual([
      { phase: "request", messages: [{ role: "user", content: [{ type: "text", text: "Mail ada@example.com" }] }] },
      {
        phase: "reply",
        messages: [{ role: "user", content: [{ type: "text", text: "Mail ada@example.com" }] }],
        reply: { role: "assistant", content: [{ type: "text", text: "ada@example.com" }] },
      },
      {
        phase: "reply",
        messages: [{ role: "user", content: [{ type: "text", text: "Mail ada@example.com" }] }],
        reply: { role: "assistant", content: [{ type: "text", text: "ada@example.com" }] },
      },
    ]);
    await user.click(within(dialog).getAllByRole("button", { name: "Close" }).at(-1) as HTMLElement);
    expect(screen.queryByRole("dialog", { name: "Test PII check" })).not.toBeInTheDocument();
  });

  it("names every failure a test can show", async () => {
    signedIn();
    on("GET", LIST, paged([guardrail()]));
    const failures = ["timeout", "unreachable", "refused", "invalid"] as const;
    let n = 0;
    on("POST", `${ONE}/test`, () => ({
      action: n === 3 ? "allow" : null,
      category: null,
      message: null,
      messages: null,
      failure: failures[n++],
      status: null,
      duration_ms: 1,
      found: {},
    }));
    const { user } = await renderApp(PATH);
    await menuOf(user, "PII check");
    await user.click(await screen.findByRole("menuitem", { name: "Test" }));
    const dialog = await screen.findByRole("dialog", { name: "Test PII check" });
    expect(within(dialog).queryByRole("radio", { name: "Replies" })).not.toBeInTheDocument();
    await fill(user, within(dialog).getByLabelText("Prompt"), "Hi");
    for (const text of ["No answer in time", "Can't be reached", "Address not allowed", "No verdict in the answer"]) {
      await user.click(within(dialog).getByRole("button", { name: "Send sample" }));
      expect(await within(dialog).findByText(text)).toBeInTheDocument();
    }
    expect(within(dialog).getByText("Allowed")).toBeInTheDocument();
  });

  it("lets people without Manage guardrails only look", async () => {
    asAdmin(ADMIN_ORG_RIGHTS.filter((r) => r !== "guardrails.manage"));
    on("GET", LIST, paged([guardrail()]));
    const { user } = await renderApp(PATH);
    await rowOf("PII check");
    const add = screen.getAllByRole("button", { name: "Add guardrail" })[0] as HTMLElement;
    expect(add).toHaveAttribute("aria-disabled", "true");
    await user.click(screen.getByRole("button", { name: "Actions for PII check" }));
    for (const item of ["Edit", "Test", "New secret", "Delete"]) {
      expect(await screen.findByRole("menuitem", { name: new RegExp(item) })).toHaveAttribute("aria-disabled", "true");
    }
  });
});
