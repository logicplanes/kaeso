import { screen, waitFor, within } from "@testing-library/react";
import type { UserEvent } from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { atFixtureTime } from "~/test/clock";
import { ALL_PROJECT_RIGHTS, signedIn } from "~/test/fixtures";
import { hoverReason, notifications } from "~/test/fixtures/org";
import { apiKey, catalogueModel, limit, memberRights, projectData, projectMember, projectPath } from "~/test/fixtures/project";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

atFixtureTime();

const sonnet = catalogueModel({ effective: { ...catalogueModel().effective, display_name: "Sonnet 4.5" } });

const LIMITS = "orgs/:org/projects/:project/limits";
const LIMIT = `${LIMITS}/:limit`;
const grace = { type: "user" as const, id: "usr_grace" };
const projectBudget = limit();
const memberBudget = limit({ id: "lim_member", level: "member", amount: "20", used: "5", subject: grace, alert_thresholds: [50, 80, 100] });
const memberRate = limit({
  id: "lim_rate",
  level: "member",
  kind: "request_rate",
  period: "minute",
  amount: "60",
  used: "3",
  resets_at: null,
  model: "cm_sonnet",
  subject: grace,
  alert_thresholds: [80],
});
const totalTokens = limit({
  id: "lim_total",
  level: "member",
  kind: "token_budget",
  period: "total",
  amount: "1000000",
  used: null,
  hard: false,
  resets_at: null,
  model: "cm_retired",
  subject: grace,
});

async function memberLimits(limits = [projectBudget, memberBudget], rights: readonly string[] = ALL_PROJECT_RIGHTS) {
  signedIn({ projectRights: rights });
  projectData({ members: [projectMember()], models: [sonnet], limits });
  on("GET", "orgs/:org/projects/:project/members/:user", projectMember());
  on("GET", "orgs/:org/projects/:project/members/:user/rights", memberRights(["project.view"]));
  const view = await renderApp(projectPath("members?member=usr_grace"));
  const sheet = await screen.findByRole("dialog", { name: "Grace Hopper" });
  const section = (await within(sheet).findByRole("heading", { name: "Limits" })).closest("section") as HTMLElement;
  return { ...view, sheet, section };
}

const addButton = (section: HTMLElement) => within(section).getAllByRole("button", { name: "Add limit" }).at(-1) as HTMLElement;

async function startDraft(user: UserEvent, section: HTMLElement) {
  await user.click(within(section).getByRole("button", { name: "Add limit" }));
  return within(section).getByRole("textbox", { name: "Amount" });
}

describe("LimitsSection", () => {
  it("shows each limit's use, period, model and reset", async () => {
    const { section } = await memberLimits([memberBudget, memberRate, totalTokens]);

    const meters = await within(section).findAllByRole("meter");
    expect(meters).toHaveLength(3);
    expect(section).toHaveTextContent("$5.00 of $20.00 · Per month · All models");
    expect(section).toHaveTextContent("Resets 1 Nov");
    expect(section).toHaveTextContent("3 of 60 · Per minute · Sonnet 4.5");
    expect(section).toHaveTextContent("Over the last minute");
    expect(section).toHaveTextContent("0 of 1,000,000 · In total · cm_retired");
    expect(section).toHaveTextContent("Never resets");
    expect(requests("GET", LIMITS).map((r) => Object.fromEntries(r.query))).toEqual(expect.arrayContaining([{ user: "usr_grace" }, { level: "project" }]));
  });

  it("adds a member limit once it's below the project's", async () => {
    on("POST", LIMITS, memberBudget);
    const { user, section } = await memberLimits();

    await within(section).findAllByRole("meter");
    const amount = await startDraft(user, section);
    expect(within(section).queryByText("No limits on this member; the project's limits apply.")).not.toBeInTheDocument();
    await hoverReason(user, addButton(section), "Enter an amount");
    await user.type(amount, "0");
    await hoverReason(user, addButton(section), "Enter an amount above zero.");
    await user.clear(amount);
    await user.type(amount, "60");
    await hoverReason(
      user,
      addButton(section),
      "Higher than the project's monthly spend budget of 50 USD; a lower level can't be higher than the one above it",
    );
    await user.clear(amount);
    await user.type(amount, "40");
    await waitFor(() => expect(addButton(section)).not.toHaveAttribute("aria-disabled", "true"));
    await user.click(addButton(section));

    expect(await notifications().findByText("Limit added")).toBeInTheDocument();
    expect(requests("POST", LIMITS).map((r) => r.body)).toEqual([
      { level: "member", user: "usr_grace", kind: "spend_budget", amount: "40", period: "month", hard: true, alert_thresholds: [50, 80, 100] },
    ]);
    await waitFor(() => expect(within(section).queryByRole("textbox", { name: "Amount" })).not.toBeInTheDocument());
  });

  it("drafts a per-model token budget that only alerts", async () => {
    on("POST", LIMITS, totalTokens);
    const { user, section } = await memberLimits([]);

    expect(await within(section).findByText("No limits on this member; the project's limits apply.")).toBeInTheDocument();
    const amount = await startDraft(user, section);
    await user.click(within(section).getByRole("combobox", { name: "What it limits" }));
    await user.click(await screen.findByRole("option", { name: /Token budget/ }));
    await user.type(amount, "1.5");
    await hoverReason(user, addButton(section), "Requests and tokens are whole numbers.");
    await user.clear(amount);
    await user.type(amount, "1000");
    await user.click(within(section).getByRole("radio", { name: "Total" }));
    await user.click(within(section).getByRole("button", { name: "Model" }));
    await user.click(await screen.findByRole("option", { name: /Sonnet 4.5/ }));
    const hard = within(section).getByRole("switch", { name: /Refuse requests when used up/ });
    expect(within(section).getByText("A hard budget: requests are refused once it is used up.")).toBeInTheDocument();
    await user.click(hard);
    expect(within(section).getByText("Alerts only: requests continue past it.")).toBeInTheDocument();
    await user.click(within(section).getByRole("button", { name: "Remove 50" }));
    const alerts = within(section).getByRole("textbox", { name: "Alert at" });
    await user.type(alerts, "abc{Enter}");
    expect(within(section).queryByRole("button", { name: "Remove abc" })).not.toBeInTheDocument();
    await user.clear(alerts);
    await user.type(alerts, "150{Enter}");
    expect(await within(section).findByText("A whole percent between 1 and 100")).toBeInTheDocument();
    await hoverReason(user, addButton(section), "Alert thresholds are between 1 and 100 percent.");
    await user.click(within(section).getByRole("button", { name: "Remove 150" }));
    await user.type(alerts, "90{Enter}");
    await user.click(addButton(section));

    await waitFor(() =>
      expect(requests("POST", LIMITS).map((r) => r.body)).toEqual([
        {
          level: "member",
          user: "usr_grace",
          kind: "token_budget",
          amount: "1000",
          period: "total",
          model: "cm_sonnet",
          hard: false,
          alert_thresholds: [80, 90, 100],
        },
      ]),
    );
  });

  it("sends a rate limit per minute and always hard", async () => {
    on("POST", LIMITS, memberRate);
    const { user, section } = await memberLimits([]);

    const amount = await startDraft(user, section);
    await user.click(within(section).getByRole("radio", { name: "Day" }));
    await user.click(within(section).getByRole("switch", { name: /Refuse requests when used up/ }));
    await user.click(within(section).getByRole("combobox", { name: "What it limits" }));
    await user.click(await screen.findByRole("option", { name: /Request rate/ }));
    expect(within(section).getByText("Rates count over a sliding minute and always refuse requests above them.")).toBeInTheDocument();
    expect(within(section).queryByRole("radio", { name: "Day" })).not.toBeInTheDocument();
    expect(within(section).queryByRole("switch")).not.toBeInTheDocument();
    expect(within(section).queryByRole("textbox", { name: "Alert at" })).not.toBeInTheDocument();
    await user.type(amount, "120");
    await user.click(within(section).getByRole("combobox", { name: "What it limits" }));
    await user.click(await screen.findByRole("option", { name: /Spend budget/ }));
    expect(within(section).getByRole("radio", { name: "Month" })).toHaveAttribute("aria-checked", "true");
    await user.click(within(section).getByRole("combobox", { name: "What it limits" }));
    await user.click(await screen.findByRole("option", { name: /Token rate/ }));
    await user.click(addButton(section));

    await waitFor(() =>
      expect(requests("POST", LIMITS).map((r) => r.body)).toEqual([
        { level: "member", user: "usr_grace", kind: "token_rate", amount: "120", period: "minute", hard: true, alert_thresholds: [] },
      ]),
    );
  });

  it("drops the draft on cancel and shows the API's message when adding fails", async () => {
    on("POST", LIMITS, apiError(422, "invalid_request", "The amount is too large.", { param: "amount" }));
    const { user, section } = await memberLimits([]);

    await startDraft(user, section);
    const cancel = within(section).getByRole("button", { name: "Cancel" });
    await user.click(cancel);
    expect(within(section).queryByRole("textbox", { name: "Amount" })).not.toBeInTheDocument();
    const amount = await startDraft(user, section);
    await user.type(amount, "10");
    await user.click(addButton(section));
    expect((await notifications().findAllByText("The amount is too large.")).length).toBeGreaterThan(0);
    expect(within(section).getByRole("textbox", { name: "Amount" })).toHaveValue("10");
  });

  it("edits a limit's amount, alerts and hardness", async () => {
    on("PATCH", LIMIT, memberBudget);
    const { user, section } = await memberLimits();

    await user.click(await within(section).findByRole("button", { name: "Edit the monthly spend budget of 20 USD" }));
    const dialog = await screen.findByRole("dialog", { name: "Edit limit" });
    expect(within(dialog).getByText("The monthly spend budget of 20 USD; to change its kind, period or model, remove it and add another.")).toBeInTheDocument();
    expect(within(dialog).queryByRole("combobox", { name: "What it limits" })).not.toBeInTheDocument();
    expect(within(dialog).queryByRole("radio", { name: "Day" })).not.toBeInTheDocument();
    const amount = within(dialog).getByRole("textbox", { name: "Amount" });
    await user.clear(amount);
    await hoverReason(user, within(dialog).getByRole("button", { name: "Save limit" }), "Enter an amount above zero.");
    await user.type(amount, "25");
    await user.click(within(dialog).getByRole("switch"));
    await user.keyboard("{Meta>}{Enter}{/Meta}");

    expect(await notifications().findByText("Limit saved")).toBeInTheDocument();
    expect(requests("PATCH", LIMIT).map((r) => [r.params.limit, r.body])).toEqual([
      ["lim_member", { amount: "25", alert_thresholds: [50, 80, 100], hard: false }],
    ]);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Edit limit" })).not.toBeInTheDocument());
  });

  it("edits a rate limit without sending hardness and closes on cancel", async () => {
    on("PATCH", LIMIT, memberRate);
    const { user, section } = await memberLimits([memberRate]);

    await user.click(await within(section).findByRole("button", { name: "Edit the request rate of 60 requests per minute" }));
    let dialog = await screen.findByRole("dialog", { name: "Edit limit" });
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Edit limit" })).not.toBeInTheDocument());

    await user.click(within(section).getByRole("button", { name: "Edit the request rate of 60 requests per minute" }));
    await screen.findByRole("dialog", { name: "Edit limit" });
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Edit limit" })).not.toBeInTheDocument());

    await user.click(within(section).getByRole("button", { name: "Edit the request rate of 60 requests per minute" }));
    dialog = await screen.findByRole("dialog", { name: "Edit limit" });
    expect(within(dialog).queryByRole("textbox", { name: "Alert at" })).not.toBeInTheDocument();
    await user.clear(within(dialog).getByRole("textbox", { name: "Amount" }));
    await user.type(within(dialog).getByRole("textbox", { name: "Amount" }), "90");
    await user.click(within(dialog).getByRole("button", { name: "Save limit" }));
    await waitFor(() => expect(requests("PATCH", LIMIT).map((r) => r.body)).toEqual([{ amount: "90", alert_thresholds: [] }]));
  });

  it("removes a limit after confirming", async () => {
    on("DELETE", LIMIT, undefined);
    const { user, section } = await memberLimits();

    await user.click(await within(section).findByRole("button", { name: "Remove the monthly spend budget of 20 USD" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Remove this limit?" });
    expect(within(confirm).getByText("The monthly spend budget of 20 USD stops applying; the levels above it still do.")).toBeInTheDocument();
    await user.click(within(confirm).getByRole("button", { name: "Remove limit" }));
    expect(await notifications().findByText("Limit removed")).toBeInTheDocument();
    expect(requests("DELETE", LIMIT).map((r) => r.params.limit)).toEqual(["lim_member"]);
  });

  it("opens the removal fresh after a refused one", async () => {
    on("DELETE", LIMIT, apiError(409, "conflict", "The limit is being changed; try again."));
    const { user, section } = await memberLimits();

    await user.click(await within(section).findByRole("button", { name: "Remove the monthly spend budget of 20 USD" }));
    let confirm = await screen.findByRole("alertdialog", { name: "Remove this limit?" });
    await user.click(within(confirm).getByRole("button", { name: "Remove limit" }));
    expect(await notifications().findByText("The limit is being changed; try again.")).toBeInTheDocument();
    await waitFor(() => expect(within(confirm).getByRole("button", { name: "Cancel" })).toBeEnabled());
    await user.click(within(confirm).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());

    on("DELETE", LIMIT, undefined);
    await user.click(within(section).getByRole("button", { name: "Remove the monthly spend budget of 20 USD" }));
    confirm = await screen.findByRole("alertdialog", { name: "Remove this limit?" });
    expect(within(confirm).getByRole("button", { name: "Remove limit" })).not.toHaveAttribute("aria-busy");
    expect(within(confirm).getByRole("button", { name: "Cancel" })).toBeEnabled();
    await user.click(within(confirm).getByRole("button", { name: "Remove limit" }));
    expect(await notifications().findByText("Limit removed")).toBeInTheDocument();
    expect(requests("DELETE", LIMIT).map((r) => r.params.limit)).toEqual(["lim_member", "lim_member"]);
  });

  it("only shows the limits without Manage limits", async () => {
    const { section } = await memberLimits(
      [memberBudget],
      ALL_PROJECT_RIGHTS.filter((r) => r !== "limits.manage"),
    );
    expect(await within(section).findByRole("meter")).toBeInTheDocument();
    expect(within(section).queryByRole("button", { name: "Add limit" })).not.toBeInTheDocument();
    expect(within(section).queryByRole("button", { name: /Edit the/ })).not.toBeInTheDocument();
  });

  it("checks a key's limit against its owner's and the project's", async () => {
    signedIn();
    projectData({
      keys: [apiKey()],
      limits: [projectBudget, limit({ id: "lim_owner", level: "member", amount: "10", subject: { type: "user", id: "usr_ada" } })],
    });
    on("GET", "orgs/:org/projects/:project/keys/:key", apiKey());
    const { user } = await renderApp(projectPath("keys?key=key_web"));

    const sheet = await screen.findByRole("dialog", { name: /checkout-web/ });
    const section = (await within(sheet).findByRole("heading", { name: "Limits" })).closest("section") as HTMLElement;
    expect(await within(section).findByText("No limits on this key; its owner's and the project's limits apply.")).toBeInTheDocument();
    await waitFor(() =>
      expect(requests("GET", LIMITS).map((r) => Object.fromEntries(r.query))).toEqual(
        expect.arrayContaining([{ key: "key_web" }, { level: "project" }, { user: "usr_ada", level: "member" }]),
      ),
    );
    const amount = await startDraft(user, section);
    await user.type(amount, "15");
    await hoverReason(user, addButton(section), "Higher than the member's monthly spend budget of 10 USD; a lower level can't be higher than the one above it");
  });
});
