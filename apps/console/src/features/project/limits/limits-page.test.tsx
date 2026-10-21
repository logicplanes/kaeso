import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { USER, signedIn, without } from "~/test/fixtures";
import { hoverReason, notifications } from "~/test/fixtures/org";
import { apiKey, catalogueModel, limit, projectData, projectMember, projectPath, serviceAccount } from "~/test/fixtures/project";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const LIMITS = "orgs/:org/projects/:project/limits";
const ONE_LIMIT = "orgs/:org/projects/:project/limits/:limit";
const cells = () => screen.getAllByRole("gridcell").map((c) => c.textContent);

const everyLevel = () => [
  limit(),
  limit({
    id: "lim_rate",
    level: "member",
    kind: "request_rate",
    period: "minute",
    amount: "60",
    used: "3",
    resets_at: null,
    alert_thresholds: [],
    subject: { type: "user", id: "usr_grace" },
  }),
  limit({
    id: "lim_sa",
    level: "member",
    kind: "token_budget",
    period: "total",
    amount: "100000",
    used: null,
    hard: false,
    resets_at: null,
    subject: { type: "service_account", id: "sa_deploy" },
  }),
  limit({
    id: "lim_key",
    level: "key",
    kind: "token_rate",
    period: "minute",
    amount: "1000",
    used: "0",
    subject: { type: "key", id: "key_web" },
    model: "cm_sonnet",
  }),
  limit({ id: "lim_gone", level: "key", kind: "spend_budget", amount: "0", subject: { type: "key", id: "key_gone" } }),
];

function stubProject(overrides: Parameters<typeof projectData>[0] = {}) {
  projectData({
    members: [projectMember()],
    accounts: [serviceAccount()],
    keys: [apiKey()],
    models: [catalogueModel()],
    limits: everyLevel(),
    ...overrides,
  });
}

describe("LimitsPage", () => {
  it("groups the limits by level and names who each applies to", async () => {
    signedIn();
    stubProject();
    await renderApp(projectPath("limits"));

    expect(await screen.findByText("Grace Hopper")).toBeInTheDocument();
    expect(cells()).toEqual([
      "Project1",
      expect.stringMatching(/^Spend budgetHardPer month · All models\$12\.50 of \$50\.00Resets /),
      "Members and service accounts2",
      "Grace HopperHardRequest rate · Per minute · All models3 of 60Last minute",
      "ci-deploy (service account)SoftToken budget · In total · All models0 of 100,000Never resets",
      "Keys2",
      "checkout-webHardToken rate · Per minute · Claude Sonnet 4.50 of 1,000Last minute",
      expect.stringMatching(/^key_goneHardSpend budget · Per month · All models\$12\.50 of \$0\.00Resets /),
    ]);
  });

  it("lists a limit on an archived key last, marked as no longer applying", async () => {
    signedIn();
    stubProject({
      keys: [apiKey(), apiKey({ id: "key_old", name: "old-batch", state: "archived" })],
      limits: [
        limit({ id: "lim_old", level: "key", kind: "spend_budget", amount: "20", subject: { type: "key", id: "key_old" } }),
        limit({ id: "lim_key", level: "key", kind: "token_rate", period: "minute", amount: "1000", used: "0", subject: { type: "key", id: "key_web" } }),
      ],
    });
    const { user } = await renderApp(projectPath("limits?level=key"));

    expect(await screen.findByText("old-batch")).toBeInTheDocument();
    const rows = cells().filter((c) => c !== "Keys2");
    expect(rows[0]).toMatch(/^checkout-web/);
    expect(rows[1]).toMatch(/^old-batchHardKey archived/);
    expect(rows[0]).not.toContain("Key archived");
    await hoverReason(user, screen.getByText("Key archived"), "The key is archived, so this limit no longer applies to any request.");
  });

  it("shows one level from the URL and switches levels with the tabs", async () => {
    signedIn();
    stubProject();
    const { user, router } = await renderApp(projectPath("limits?level=member"));

    expect(await screen.findByText("Grace Hopper")).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Members" })).toHaveAttribute("aria-selected", "true");
    expect(screen.queryByText("Spend budget")).not.toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "Keys" }));
    expect(await screen.findByText("checkout-web")).toBeInTheDocument();
    expect(router.state.location.search).toEqual({ level: "key" });
    await user.click(screen.getByRole("tab", { name: "All" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
    expect(await screen.findByText("Grace Hopper")).toBeInTheDocument();
  });

  it("says when a level has no limits yet", async () => {
    signedIn();
    stubProject({ limits: [] });
    const { user } = await renderApp(projectPath("limits?level=key"));

    expect(await screen.findByText("No key limits yet")).toBeInTheDocument();
    expect(screen.getByText("Add a spend budget or a rate limit for one key.")).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "Project" }));
    expect(await screen.findByText("No project limits yet")).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "Members" }));
    expect(await screen.findByText("No member limits yet")).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "All" }));
    expect(await screen.findByText("No limits yet")).toBeInTheDocument();
    expect(screen.getByText("Spend budgets and rate limits; a request must pass every one that applies.")).toBeInTheDocument();
  });

  it("offers to add the first limit from the empty state", async () => {
    signedIn();
    stubProject({ limits: [] });
    const { user } = await renderApp(projectPath("limits"));

    await screen.findByText("No limits yet");
    const buttons = screen.getAllByRole("button", { name: "Add limit" });
    expect(buttons).toHaveLength(2);
    await user.click(buttons[1] as HTMLElement);
    expect(await screen.findByRole("dialog", { name: /Add limit/ })).toBeInTheDocument();
  });

  it("adds a project spend budget", async () => {
    signedIn();
    stubProject({ limits: [] });
    on("POST", LIMITS, ({ body }: { body: Record<string, unknown> }) => limit({ ...body, id: "lim_new" } as never));
    const { user } = await renderApp(projectPath("limits?add=true"));

    const dialog = await screen.findByRole("dialog", { name: /Add limit/ });
    expect(within(dialog).getByText("Who does it apply to?")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByText("What's the limit?")).toBeInTheDocument();
    await hoverReason(user, within(dialog).getByRole("button", { name: "Add limit" }), "Enter an amount");
    await user.type(within(dialog).getByRole("textbox", { name: "Amount" }), "25");
    await user.click(within(dialog).getByRole("button", { name: "Add limit" }));

    expect(await notifications().findByText("Added the monthly spend budget of 25 USD")).toBeInTheDocument();
    expect(requests("POST", LIMITS).map((r) => r.body)).toEqual([
      { level: "project", kind: "spend_budget", amount: "25", period: "month", hard: true, alert_thresholds: [50, 80, 100] },
    ]);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: /Add limit/ })).not.toBeInTheDocument());
  });

  it("adds a member rate limit for one model and refuses a budget above the project's", async () => {
    signedIn();
    stubProject({ limits: [limit({ amount: "50" })] });
    on("POST", LIMITS, ({ body }: { body: Record<string, unknown> }) => limit({ ...body, id: "lim_new", level: "member" } as never));
    const { user } = await renderApp(projectPath("limits?level=member&add=true"));

    const dialog = await screen.findByRole("dialog", { name: /Add limit/ });
    expect(within(dialog).getByRole("radio", { name: "Member" })).toHaveAttribute("aria-checked", "true");
    await hoverReason(user, within(dialog).getByRole("button", { name: "Continue" }), "Choose a member or service account");
    await user.click(within(dialog).getByRole("combobox", { name: "Member or service account" }));
    expect(await screen.findByRole("option", { name: /ci-deploy/ })).toBeInTheDocument();
    await user.click(screen.getByRole("option", { name: /Grace Hopper/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await within(dialog).findByText("What's the limit?");

    await user.type(within(dialog).getByRole("textbox", { name: "Amount" }), "80");
    await hoverReason(
      user,
      within(dialog).getByRole("button", { name: "Add limit" }),
      "Higher than the project's monthly spend budget of 50 USD; a lower level can't be higher than the one above it",
    );
    await user.click(within(dialog).getByRole("combobox", { name: "What it limits" }));
    await user.click(await screen.findByRole("option", { name: /Request rate/ }));
    expect(within(dialog).getByText("Rates count over a sliding minute and always refuse requests above them.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Model" }));
    await user.click(await screen.findByRole("option", { name: /Claude Sonnet 4.5/ }));
    expect(within(dialog).queryByRole("textbox", { name: "Alert at" })).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Add limit" }));

    expect(await notifications().findByText("Added the request rate of 80 requests per minute")).toBeInTheDocument();
    expect(requests("POST", LIMITS).map((r) => r.body)).toEqual([
      { level: "member", user: "usr_grace", kind: "request_rate", amount: "80", period: "minute", model: "cm_sonnet", hard: true, alert_thresholds: [] },
    ]);
  });

  it("adds a soft daily token budget for a service account", async () => {
    signedIn();
    stubProject({ limits: [] });
    on("POST", LIMITS, ({ body }: { body: Record<string, unknown> }) => limit({ ...body, id: "lim_new", level: "member" } as never));
    const { user } = await renderApp(projectPath("limits?add=true"));

    const dialog = await screen.findByRole("dialog", { name: /Add limit/ });
    await user.click(within(dialog).getByRole("radio", { name: "Member" }));
    await user.click(within(dialog).getByRole("combobox", { name: "Member or service account" }));
    await user.click(await screen.findByRole("option", { name: /ci-deploy/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await within(dialog).findByText("What's the limit?");
    await user.click(within(dialog).getByRole("combobox", { name: "What it limits" }));
    await user.click(await screen.findByRole("option", { name: /Token budget/ }));
    await user.click(within(dialog).getByRole("radio", { name: "Day" }));
    await user.type(within(dialog).getByRole("textbox", { name: "Amount" }), "1.5");
    await hoverReason(user, within(dialog).getByRole("button", { name: "Add limit" }), "Requests and tokens are whole numbers.");
    await user.clear(within(dialog).getByRole("textbox", { name: "Amount" }));
    await user.type(within(dialog).getByRole("textbox", { name: "Amount" }), "5000");
    expect(within(dialog).getByText("A hard budget: requests are refused once it is used up.")).toBeInTheDocument();
    const hard = within(dialog).getByRole("switch");
    await user.click(hard);
    expect(within(dialog).getByText("Alerts only: requests continue past it.")).toBeInTheDocument();
    await user.type(within(dialog).getByRole("textbox", { name: "Alert at" }), "25{Enter}");
    await user.click(within(dialog).getByRole("button", { name: "Add limit" }));

    expect(await notifications().findByText("Added the daily token budget of 5000 tokens")).toBeInTheDocument();
    expect(requests("POST", LIMITS).map((r) => r.body)).toEqual([
      { level: "member", service_account: "sa_deploy", kind: "token_budget", amount: "5000", period: "day", hard: false, alert_thresholds: [25, 50, 80, 100] },
    ]);
  });

  it("adds a key limit checked against the key owner's limits", async () => {
    signedIn();
    stubProject({
      keys: [
        apiKey(),
        apiKey({ id: "key_sa", name: "deploy-key", principal: { kind: "service_account", id: "sa_deploy" } }),
        apiKey({ id: "key_old", name: "old-key", state: "archived" }),
      ],
      limits: [limit({ id: "lim_sa", level: "member", amount: "10", subject: { type: "service_account", id: "sa_deploy" } })],
    });
    const { user } = await renderApp(projectPath("limits?level=key&add=true"));

    const dialog = await screen.findByRole("dialog", { name: /Add limit/ });
    await hoverReason(user, within(dialog).getByRole("button", { name: "Continue" }), "Choose a key");
    await user.click(within(dialog).getByRole("combobox", { name: "Key" }));
    expect(await screen.findByRole("option", { name: /checkout-web/ })).toBeInTheDocument();
    expect(screen.queryByRole("option", { name: /old-key/ })).not.toBeInTheDocument();
    await user.click(screen.getByRole("option", { name: /deploy-key/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await within(dialog).findByText("What's the limit?");
    await waitFor(() =>
      expect(requests("GET", LIMITS).map((r) => Object.fromEntries(r.query))).toContainEqual({ level: "member", service_account: "sa_deploy" }),
    );
    await user.type(within(dialog).getByRole("textbox", { name: "Amount" }), "20");
    await hoverReason(user, within(dialog).getByRole("button", { name: "Add limit" }), /Higher than the member's monthly spend budget of 10 USD/);
    expect(requests("POST", LIMITS)).toHaveLength(0);
  });

  it("opens adding from the header and the shortcut, and closes it with Escape", async () => {
    signedIn();
    stubProject();
    const { user } = await renderApp(projectPath("limits"));

    await screen.findByText("Grace Hopper");
    await user.click(screen.getByRole("button", { name: "Add limit" }));
    expect(await screen.findByRole("dialog", { name: /Add limit/ })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: /Add limit/ })).not.toBeInTheDocument());
    await user.keyboard("c");
    expect(await screen.findByRole("dialog", { name: /Add limit/ })).toBeInTheDocument();
  });

  it("checks a key limit against the limits of the person whose key it is", async () => {
    signedIn();
    stubProject({ members: [projectMember({ user: { email: null } })], limits: [] });
    const { user } = await renderApp(projectPath("limits?level=member&add=true"));

    const dialog = await screen.findByRole("dialog", { name: /Add limit/ });
    await user.click(within(dialog).getByRole("combobox", { name: "Member or service account" }));
    expect(await screen.findByRole("option", { name: "Grace Hopper" })).toHaveTextContent(/^Grace Hopper$/);
    await user.keyboard("{Escape}");
    await user.click(within(dialog).getByRole("radio", { name: "Key" }));
    await user.click(within(dialog).getByRole("combobox", { name: "Key" }));
    await user.click(await screen.findByRole("option", { name: /checkout-web/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await within(dialog).findByText("What's the limit?");
    await waitFor(() => expect(requests("GET", LIMITS).map((r) => Object.fromEntries(r.query))).toContainEqual({ level: "member", user: USER }));
    const model = within(dialog).getByRole("button", { name: "Model" });
    await user.click(model);
    await user.click(await screen.findByRole("option", { name: /Claude Sonnet 4.5/ }));
    expect(model).toHaveTextContent("Claude Sonnet 4.5");
    await user.click(model);
    await user.click(await screen.findByRole("option", { name: "All models" }));
    expect(model).toHaveTextContent("All models");
  });

  it("shows the API's message when adding is refused", async () => {
    signedIn();
    stubProject({ limits: [] });
    on("POST", LIMITS, apiError(422, "invalid_request", "That limit already exists."));
    const { user } = await renderApp(projectPath("limits?add=true"));

    const dialog = await screen.findByRole("dialog", { name: /Add limit/ });
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await within(dialog).findByText("What's the limit?");
    await user.type(within(dialog).getByRole("textbox", { name: "Amount" }), "25{Enter}");
    expect((await notifications().findAllByText("That limit already exists.")).length).toBeGreaterThan(0);
    expect(screen.getByRole("dialog", { name: /Add limit/ })).toBeInTheDocument();
  });

  it("edits a limit from its row and from its menu", async () => {
    signedIn();
    stubProject();
    on("PATCH", ONE_LIMIT, ({ body }: { body: Record<string, unknown> }) => limit(body as never));
    const { user } = await renderApp(projectPath("limits"));

    await user.click(await screen.findByRole("gridcell", { name: /^Spend budget/ }));
    let dialog = await screen.findByRole("dialog", { name: "Edit limit" });
    const amount = within(dialog).getByRole("textbox", { name: "Amount" });
    await user.clear(amount);
    await user.type(amount, "75");
    await user.click(within(dialog).getByRole("button", { name: "Save limit" }));
    expect(await notifications().findByText("Limit saved")).toBeInTheDocument();
    expect(requests("PATCH", ONE_LIMIT).map((r) => [r.params.limit, r.headers.get("if-match"), r.body])).toEqual([
      ["lim_month", '"1"', { amount: "75", alert_thresholds: [80], hard: true }],
    ]);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Edit limit" })).not.toBeInTheDocument());

    await user.click(screen.getByRole("button", { name: "Actions for the request rate of 60 requests per minute" }));
    await user.click(await screen.findByRole("menuitem", { name: "Edit" }));
    dialog = await screen.findByRole("dialog", { name: "Edit limit" });
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Edit limit" })).not.toBeInTheDocument());
  });

  it("says when someone else changed the limit meanwhile, and reloads the limits", async () => {
    signedIn();
    stubProject();
    on("PATCH", ONE_LIMIT, apiError(412, "conflict", "This limit was changed since you loaded it; reload it and try again."));
    const { user } = await renderApp(projectPath("limits"));

    await user.click(await screen.findByRole("gridcell", { name: /^Spend budget/ }));
    const dialog = await screen.findByRole("dialog", { name: "Edit limit" });
    const amount = within(dialog).getByRole("textbox", { name: "Amount" });
    await user.clear(amount);
    await user.type(amount, "75");
    const loads = requests("GET", LIMITS).length;
    await user.click(within(dialog).getByRole("button", { name: "Save limit" }));

    const toasts = notifications();
    expect(await toasts.findByText("This limit was changed since you loaded it; reload it and try again.")).toBeInTheDocument();
    expect(toasts.getByText("The latest version is loaded now.")).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Edit limit" })).not.toBeInTheDocument());
    await waitFor(() => expect(requests("GET", LIMITS).length).toBeGreaterThan(loads));
  });

  it("keeps Remove waiting while a change to the limit is saved, so the two can't race", async () => {
    signedIn();
    stubProject();
    let respond: (body: unknown) => void = () => undefined;
    on("PATCH", ONE_LIMIT, ({ body }: { body: Record<string, unknown> }) => new Promise((resolve) => (respond = () => resolve(limit(body as never)))));
    const { user } = await renderApp(projectPath("limits"));
    await user.click(await screen.findByRole("gridcell", { name: /^Spend budget/ }));
    const dialog = await screen.findByRole("dialog", { name: "Edit limit" });
    const amount = within(dialog).getByRole("textbox", { name: "Amount" });
    await user.clear(amount);
    await user.type(amount, "75");
    await user.click(within(dialog).getByRole("button", { name: "Save limit" }));
    await waitFor(() => expect(requests("PATCH", ONE_LIMIT)).toHaveLength(1));
    expect(within(dialog).getByRole("button", { name: "Remove limit" })).toBeDisabled();
    await user.click(within(dialog).getByRole("button", { name: "Remove limit" }));
    expect(screen.queryByRole("alertdialog", { name: "Remove this limit?" })).not.toBeInTheDocument();
    respond(null);
    expect(await notifications().findByText("Limit saved")).toBeInTheDocument();
    expect(requests("DELETE", ONE_LIMIT)).toHaveLength(0);
  });

  it("keeps the edit open when a change or removal is refused for another reason", async () => {
    signedIn();
    stubProject();
    on("PATCH", ONE_LIMIT, apiError(400, "invalid_request", "`amount` must be above zero.", { param: "amount" }));
    on("DELETE", ONE_LIMIT, apiError(403, "permission_denied", "You need the right `limits.manage` in this project; ask one of its admins for it."));
    const { user } = await renderApp(projectPath("limits"));

    await user.click(await screen.findByRole("gridcell", { name: /^Spend budget/ }));
    const dialog = await screen.findByRole("dialog", { name: "Edit limit" });
    const amount = within(dialog).getByRole("textbox", { name: "Amount" });
    await user.clear(amount);
    await user.type(amount, "75");
    await user.click(within(dialog).getByRole("button", { name: "Save limit" }));
    expect((await notifications().findByText("amount", { selector: "code" })).parentElement).toHaveTextContent(/^amount must be above zero\.$/);
    expect(screen.getByRole("dialog", { name: "Edit limit" })).toBeInTheDocument();

    await user.click(within(dialog).getByRole("button", { name: "Remove limit" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Remove this limit?" });
    await user.click(within(confirm).getByRole("button", { name: "Remove limit" }));
    expect((await notifications().findByText("limits.manage", { selector: "code" })).parentElement).toHaveTextContent(
      /^You need the right limits\.manage in this project; ask one of its admins for it\.$/,
    );
    expect(requests("DELETE", ONE_LIMIT)).toHaveLength(1);
  });

  it("closes the removal when the limit changed meanwhile", async () => {
    signedIn();
    stubProject();
    on("DELETE", ONE_LIMIT, apiError(412, "conflict", "This limit was changed since you loaded it; reload it and try again."));
    const { user } = await renderApp(projectPath("limits"));

    await user.click(await screen.findByRole("button", { name: "Actions for the monthly spend budget of 50 USD" }));
    await user.click(await screen.findByRole("menuitem", { name: "Remove" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Remove this limit?" });
    await user.click(within(confirm).getByRole("button", { name: "Remove limit" }));
    expect(await notifications().findByText("This limit was changed since you loaded it; reload it and try again.")).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByRole("alertdialog", { name: "Remove this limit?" })).not.toBeInTheDocument());
  });

  it("removes a limit from its menu and from the edit dialog", async () => {
    signedIn();
    stubProject();
    on("DELETE", ONE_LIMIT, () => undefined);
    const { user } = await renderApp(projectPath("limits"));

    await user.click(await screen.findByRole("button", { name: "Actions for the monthly spend budget of 50 USD" }));
    await user.click(await screen.findByRole("menuitem", { name: "Remove" }));
    let confirm = await screen.findByRole("alertdialog", { name: "Remove this limit?" });
    expect(within(confirm).getByText("The monthly spend budget of 50 USD stops applying; the levels above it still do.")).toBeInTheDocument();
    await user.click(within(confirm).getByRole("button", { name: "Remove limit" }));
    expect(await notifications().findByText("Limit removed")).toBeInTheDocument();
    expect(requests("DELETE", ONE_LIMIT).map((r) => [r.params.limit, r.headers.get("if-match")])).toEqual([["lim_month", '"1"']]);

    await user.click(screen.getByRole("gridcell", { name: /^Grace Hopper/ }));
    const dialog = await screen.findByRole("dialog", { name: "Edit limit" });
    await user.click(within(dialog).getByRole("button", { name: "Remove limit" }));
    confirm = await screen.findByRole("alertdialog", { name: "Remove this limit?" });
    await user.click(within(confirm).getByRole("button", { name: "Remove limit" }));
    await waitFor(() => expect(requests("DELETE", ONE_LIMIT).map((r) => r.params.limit)).toEqual(["lim_month", "lim_rate"]));
  });

  it("shows limits read-only and explains why they can't be changed without the right", async () => {
    signedIn({ projectRights: without("limits.manage", "keys.all.view") });
    stubProject();
    const { user } = await renderApp(projectPath("limits"));

    expect(await screen.findByText("Grace Hopper")).toBeInTheDocument();
    expect(screen.getByText("key_web")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Actions for/ })).not.toBeInTheDocument();
    const add = screen.getByRole("button", { name: "Add limit" });
    expect(add).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, add, /You need Manage limits/);
    await user.click(screen.getByRole("gridcell", { name: /^Spend budget/ }));
    expect(screen.queryByRole("dialog", { name: "Edit limit" })).not.toBeInTheDocument();
  });

  it("offers no add button in the empty state without the right", async () => {
    signedIn({ projectRights: without("limits.manage") });
    stubProject({ limits: [] });
    await renderApp(projectPath("limits"));

    expect(await screen.findByText("No limits yet")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "Add limit" })).toHaveLength(1);
  });
});
