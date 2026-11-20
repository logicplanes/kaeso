import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import type { WebhookDeliveryView, WebhookView } from "~/api/generated/types";
import { ORG, PROJECT, project, signedIn, withRights } from "~/test/fixtures";
import { fill, hoverReason, notifications, paged } from "~/test/fixtures/org";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = `/settings/orgs/${ORG}/webhooks`;
const NOW = new Date().toISOString();

function webhook(overrides: Partial<WebhookView> = {}): WebhookView {
  return {
    id: "whk_oncall",
    name: "On-call",
    url: "https://hooks.example.com/kaeso",
    format: "json",
    kinds: ["budget_alerts", "provider_keys"],
    project: null,
    enabled: true,
    disabled_reason: null,
    consecutive_failures: 0,
    last_status: null,
    last_error: null,
    last_attempt_at: null,
    created_at: NOW,
    updated_at: NOW,
    ...overrides,
  };
}

function delivery(overrides: Partial<WebhookDeliveryView> = {}): WebhookDeliveryView {
  return {
    id: "msg_1",
    type: "budget.alert",
    status: "delivered",
    attempts: 1,
    last_status: 204,
    last_error: null,
    next_attempt_at: null,
    created_at: NOW,
    finished_at: NOW,
    ...overrides,
  };
}

async function cellOf(name: string) {
  const grid = await screen.findByRole("grid", { name: "Webhooks" });
  return within(grid).findByRole("gridcell", { name: new RegExp(name) });
}

describe("webhooks", () => {
  it("lists the webhooks with their state and their last delivery", async () => {
    signedIn();
    on(
      "GET",
      "orgs/:org/webhooks",
      paged([
        webhook({ last_attempt_at: NOW, last_status: 204 }),
        webhook({ id: "whk_chat", name: "Chat", format: "slack", enabled: false, last_attempt_at: NOW, last_error: "The receiver answered 500." }),
        webhook({ id: "whk_flaky", name: "Flaky", enabled: false, disabled_reason: "failures" }),
      ]),
    );
    await renderApp(PATH);
    expect(await screen.findByRole("heading", { name: "Webhooks", level: 1 })).toBeInTheDocument();
    const oncall = await cellOf("On-call");
    expect(oncall).toHaveTextContent("https://hooks.example.com/kaeso");
    expect(oncall).toHaveTextContent("On");
    expect(oncall).toHaveTextContent("Last delivered");
    const chat = await cellOf("Chat");
    expect(chat).toHaveTextContent("Off");
    expect(chat).toHaveTextContent("Last try failed");
    const flaky = await cellOf("Flaky");
    expect(flaky).toHaveTextContent("Off after failures");
    expect(flaky).toHaveTextContent("Nothing sent yet");
  });

  it("says when there are none, and when they couldn't load", async () => {
    signedIn();
    on("GET", "orgs/:org/webhooks", paged([]));
    await renderApp(PATH);
    expect(await screen.findByText("No webhooks yet")).toBeInTheDocument();
    expect(screen.getByText("Add one to post budget alerts, refused provider keys and more to a URL of yours.")).toBeInTheDocument();
  });

  it("says when the webhooks couldn't load", async () => {
    signedIn();
    on("GET", "orgs/:org/webhooks", apiError(500, "internal_error", "Kaeso had a problem."));
    await renderApp(PATH);
    expect(await screen.findByText("The webhooks couldn't load")).toBeInTheDocument();
  });

  it("hides the page from people who can't view webhooks and offers no changes without managing them", async () => {
    withRights(["org.view"], { role: "auditor" });
    await renderApp(PATH);
    expect(await screen.findByText("You can't see the webhooks here")).toBeInTheDocument();
    expect(requests("GET", "orgs/:org/webhooks")).toHaveLength(0);
  });

  it("offers no actions and no adding without the right to manage webhooks", async () => {
    withRights(["org.view", "webhooks.view"], { role: "auditor" });
    on("GET", "orgs/:org/webhooks", paged([webhook()]));
    const { user } = await renderApp(PATH);
    expect(await cellOf("On-call")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Actions for On-call" })).not.toBeInTheDocument();
    const add = screen.getByRole("button", { name: "Add webhook" });
    expect(add).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, add, /You need Manage webhooks/);
  });

  it("adds a webhook for one project and shows its secret once", async () => {
    signedIn({ projects: [project()] });
    on("GET", "orgs/:org/webhooks", paged([]));
    on("GET", "orgs/:org/projects", paged([project()]));
    on("POST", "orgs/:org/webhooks", { ...webhook({ name: "Ops", kinds: ["budget_alerts"], project: PROJECT }), secret: "whsec_c2VjcmV0" });
    const { user } = await renderApp(PATH);
    await screen.findByText("No webhooks yet");
    await user.click(screen.getAllByRole("button", { name: "Add webhook" })[0] as HTMLElement);
    const dialog = await screen.findByRole("dialog", { name: "Add webhook" });
    await hoverReason(user, within(dialog).getByRole("button", { name: "Continue" }), "Give the webhook a name");
    await fill(user, within(dialog).getByRole("textbox", { name: "Name" }), " Ops ");
    await hoverReason(user, within(dialog).getByRole("button", { name: "Continue" }), "Enter the receiver's URL");
    await fill(user, within(dialog).getByRole("textbox", { name: "URL" }), "https://hooks.example.com/ops");
    await user.click(within(dialog).getByRole("combobox", { name: "Format" }));
    await user.click(await screen.findByRole("option", { name: /^Slack/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByText("Whatever people chose for their own emails, the webhook gets these.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("checkbox", { name: "Provider keys need attention" }));
    await user.click(within(dialog).getByRole("checkbox", { name: "Budget alerts" }));
    await hoverReason(user, within(dialog).getByRole("button", { name: "Add webhook" }), "Choose at least one notice");
    await user.click(within(dialog).getByRole("checkbox", { name: "Budget alerts" }));
    await user.click(within(dialog).getByRole("combobox", { name: "Events of" }));
    await user.click(await screen.findByRole("option", { name: "Checkout assistant" }));
    await user.click(within(dialog).getByRole("button", { name: "Add webhook" }));
    const reveal = await screen.findByRole("dialog", { name: "Webhook added" });
    expect(within(reveal).getByText("Copy the signing secret for Ops now; it's shown only once.")).toBeInTheDocument();
    expect(reveal).toHaveTextContent("whsec_c2VjcmV0");
    expect(requests("POST", "orgs/:org/webhooks").map((r) => r.body)).toEqual([
      { name: "Ops", url: "https://hooks.example.com/ops", format: "slack", kinds: ["budget_alerts"], project: PROJECT },
    ]);
    await user.click(within(reveal).getByRole("button", { name: "Finish" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("adds a webhook for the whole organization with the notices it starts with", async () => {
    signedIn();
    on("GET", "orgs/:org/webhooks", paged([]));
    // Without the projects, the webhook can still take the whole organization's events.
    on("GET", "orgs/:org/projects", apiError(500, "internal_error", "Kaeso had a problem."));
    on("POST", "orgs/:org/webhooks", { ...webhook(), secret: "whsec_c2VjcmV0" });
    const { user } = await renderApp(PATH);
    await screen.findByText("No webhooks yet");
    // `c` adds a webhook, as on every page that adds things.
    await user.keyboard("c");
    const dialog = await screen.findByRole("dialog", { name: "Add webhook" });
    await fill(user, within(dialog).getByRole("textbox", { name: "Name" }), "On-call");
    await fill(user, within(dialog).getByRole("textbox", { name: "URL" }), "https://hooks.example.com/kaeso");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(await within(dialog).findByRole("button", { name: "Add webhook" }));
    await screen.findByRole("dialog", { name: "Webhook added" });
    expect(requests("POST", "orgs/:org/webhooks").map((r) => r.body)).toEqual([
      { name: "On-call", url: "https://hooks.example.com/kaeso", format: "json", kinds: ["budget_alerts", "provider_keys"] },
    ]);
  });

  it("closes the dialog without adding a webhook", async () => {
    signedIn();
    on("GET", "orgs/:org/webhooks", paged([]));
    on("GET", "orgs/:org/projects", paged([]));
    const { user } = await renderApp(PATH);
    await screen.findByText("No webhooks yet");
    await user.click(screen.getAllByRole("button", { name: "Add webhook" })[0] as HTMLElement);
    await screen.findByRole("dialog", { name: "Add webhook" });
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(requests("POST", "orgs/:org/webhooks")).toHaveLength(0);
  });

  it("sends a test and says how it went", async () => {
    signedIn();
    on("GET", "orgs/:org/webhooks", paged([webhook()]));
    on("POST", "orgs/:org/webhooks/:webhook/test", delivery({ type: "webhook.test" }));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Actions for On-call" }));
    await user.click(await screen.findByRole("menuitem", { name: "Send test" }));
    expect(await notifications().findByText("Test delivered")).toBeInTheDocument();
    on(
      "POST",
      "orgs/:org/webhooks/:webhook/test",
      delivery({ type: "webhook.test", status: "failed", last_status: 500, last_error: "The receiver answered 500." }),
    );
    await user.click(screen.getByRole("button", { name: "Actions for On-call" }));
    await user.click(await screen.findByRole("menuitem", { name: "Send test" }));
    expect(await notifications().findByText("The test didn't arrive; its delivery below says why.")).toBeInTheDocument();
    expect(requests("POST", "orgs/:org/webhooks/whk_oncall/test")).toHaveLength(2);
  });

  it("turns a webhook off and on, and offers no test while it is off", async () => {
    signedIn();
    on("GET", "orgs/:org/webhooks", paged([webhook()]));
    on("PATCH", "orgs/:org/webhooks/:webhook", webhook({ enabled: false }));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Actions for On-call" }));
    // The list reloads after the change, here as one Kaeso turned off after failures.
    on("GET", "orgs/:org/webhooks", paged([webhook({ enabled: false, disabled_reason: "failures" })]));
    await user.click(await screen.findByRole("menuitem", { name: "Turn off" }));
    expect(await notifications().findByText("On-call turned off")).toBeInTheDocument();
    expect(requests("PATCH", "orgs/:org/webhooks/whk_oncall").map((r) => r.body)).toEqual([{ enabled: false }]);

    on("PATCH", "orgs/:org/webhooks/:webhook", webhook());
    await waitFor(async () => expect(await cellOf("On-call")).toHaveTextContent("Off after failures"));
    await user.click(screen.getByRole("button", { name: "Actions for On-call" }));
    expect(await screen.findByRole("menuitem", { name: /^Send test/ })).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByText("Turn it on to send a test.")).toBeInTheDocument();
    await user.click(screen.getByRole("menuitem", { name: "Turn on" }));
    expect(await notifications().findByText("On-call turned on")).toBeInTheDocument();
  });

  it("rotates the secret and shows the new one once", async () => {
    signedIn();
    on("GET", "orgs/:org/webhooks", paged([webhook()]));
    on("POST", "orgs/:org/webhooks/:webhook/secret", { secret: "whsec_bmV3" });
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Actions for On-call" }));
    await user.click(await screen.findByRole("menuitem", { name: "Rotate secret" }));
    const dialog = await screen.findByRole("dialog", { name: "Rotate the secret of On-call" });
    expect(within(dialog).getByText("The old secret stops signing at once, so its receiver rejects deliveries until updated.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Rotate secret" }));
    const reveal = await screen.findByRole("dialog", { name: "New signing secret" });
    expect(reveal).toHaveTextContent("whsec_bmV3");
    await user.click(within(reveal).getByRole("button", { name: "Finish" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("closes the rotation without rotating", async () => {
    signedIn();
    on("GET", "orgs/:org/webhooks", paged([webhook()]));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Actions for On-call" }));
    await user.click(await screen.findByRole("menuitem", { name: "Rotate secret" }));
    await screen.findByRole("dialog", { name: "Rotate the secret of On-call" });
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(requests("POST", "orgs/:org/webhooks/:webhook/secret")).toHaveLength(0);
  });

  it("deletes a webhook once confirmed, and closes the confirmation when it is refused", async () => {
    signedIn();
    on("GET", "orgs/:org/webhooks", paged([webhook()]));
    on("DELETE", "orgs/:org/webhooks/:webhook", null);
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Actions for On-call" }));
    await user.click(await screen.findByRole("menuitem", { name: "Delete" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Delete On-call?" });
    expect(within(confirm).getByText("Events no longer go to it, and its deliveries are gone.")).toBeInTheDocument();
    await user.click(within(confirm).getByRole("button", { name: "Delete webhook" }));
    expect(await notifications().findByText("On-call deleted")).toBeInTheDocument();
    expect(requests("DELETE", "orgs/:org/webhooks/whk_oncall")).toHaveLength(1);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());

    on("DELETE", "orgs/:org/webhooks/:webhook", apiError(404, "not_found", "No webhook with that id."));
    await user.click(screen.getByRole("button", { name: "Actions for On-call" }));
    await user.click(await screen.findByRole("menuitem", { name: "Delete" }));
    await user.click(within(await screen.findByRole("alertdialog", { name: "Delete On-call?" })).getByRole("button", { name: "Delete webhook" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });

  it("opens a webhook with what it receives and its last deliveries", async () => {
    signedIn({ projects: [project()] });
    on("GET", "orgs/:org/webhooks", paged([webhook({ project: PROJECT, format: "slack" }), webhook({ id: "whk_all", name: "All" })]));
    on("GET", "orgs/:org/projects", paged([project()]));
    on("GET", "orgs/:org/webhooks/:webhook/deliveries", ({ params }: { params: Record<string, string> }) =>
      params.webhook === "whk_oncall"
        ? paged([
            delivery(),
            delivery({ id: "msg_2", type: "provider_key.refused", status: "failed", attempts: 5, last_status: 500, last_error: "The receiver answered 500." }),
            delivery({ id: "msg_3", type: "limit.reached", status: "pending", attempts: 2, last_status: null, last_error: null }),
          ])
        : paged([]),
    );
    const { user } = await renderApp(PATH);
    await user.click(await cellOf("On-call"));
    const sheet = await screen.findByRole("dialog", { name: "On-call" });
    expect(sheet).toHaveTextContent("Checkout assistant");
    expect(sheet).toHaveTextContent("Slack");
    expect(sheet).toHaveTextContent("Budget alerts, Provider keys need attention");
    // A long URL or list of notices wraps in the sheet instead of running past a phone's edge, and the URL can be copied.
    const url = within(sheet).getByText("https://hooks.example.com/kaeso");
    expect(url).toHaveClass("break-words");
    expect(url.closest("dd")).toHaveClass("mono");
    expect(within(url.closest("dd")!).getByRole("button", { name: /Copy/ })).toBeInTheDocument();
    expect(within(sheet).getByText("Budget alerts, Provider keys need attention")).toHaveClass("break-words");
    const list = await within(sheet).findByRole("grid", { name: "Last deliveries" });
    await waitFor(() => expect(within(list).getAllByRole("row")).toHaveLength(3));
    expect(list).toHaveTextContent("Delivered · 204");
    expect(list).toHaveTextContent("Failed · 500");
    expect(list).toHaveTextContent("The receiver answered 500.");
    expect(list).toHaveTextContent("Waiting to try again");
    expect(list).toHaveTextContent("2 tries");

    await user.click(await cellOf("All"));
    const other = await screen.findByRole("dialog", { name: "All" });
    expect(other).toHaveTextContent("The whole organization");
    expect(await within(other).findByText("No deliveries in the last 7 days")).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "All" })).not.toBeInTheDocument());
  });

  it("says when the deliveries couldn't load", async () => {
    signedIn();
    on("GET", "orgs/:org/webhooks", paged([webhook({ project: "proj_gone" })]));
    on("GET", "orgs/:org/projects", paged([]));
    on("GET", "orgs/:org/webhooks/:webhook/deliveries", apiError(500, "internal_error", "Kaeso had a problem."));
    const { user } = await renderApp(PATH);
    await user.click(await cellOf("On-call"));
    const sheet = await screen.findByRole("dialog", { name: "On-call" });
    expect(sheet).toHaveTextContent("proj_gone");
    expect(await within(sheet).findByText("The deliveries couldn't load")).toBeInTheDocument();
  });
});
