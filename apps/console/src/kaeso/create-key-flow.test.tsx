import { screen, waitFor, within } from "@testing-library/react";
import type { UserEvent } from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { org, project, signedIn, without } from "~/test/fixtures";
import { hoverReason, notifications } from "~/test/fixtures/org";
import { access, apiKey, catalogueModel, projectData, projectPath, serviceAccount } from "~/test/fixtures/project";
import { createdKey, model } from "~/test/fixtures/shell";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const sonnet = catalogueModel({ effective: { ...catalogueModel().effective, display_name: "Sonnet 4.5" } });

const KEYS = "orgs/:org/projects/:project/keys";
const SECRET = createdKey().secret;

async function openDialog(path = "keys?create=true") {
  const view = await renderApp(projectPath(path));
  const dialog = await screen.findByRole("dialog", { name: /Create key/ });
  return { ...view, dialog };
}

async function next(user: UserEvent, dialog: HTMLElement, title: string) {
  await user.click(within(dialog).getByRole("button", { name: "Continue" }));
  expect(await within(dialog).findByRole("heading", { name: title })).toBeVisible();
}

describe("CreateKeyDialog", () => {
  it("creates an inference key for you with the default expiry and shows it once", async () => {
    signedIn();
    projectData();
    on("POST", KEYS, createdKey());
    const { user, dialog } = await openDialog();

    expect(within(dialog).getByRole("heading", { name: "What is it for?" })).toBeInTheDocument();
    const name = within(dialog).getByRole("textbox", { name: "Name" });
    expect(name).toHaveAttribute("placeholder", "checkout-web");
    await user.type(name, "{Enter}");
    expect(within(dialog).getByRole("heading", { name: "What is it for?" })).toBeVisible();
    await user.type(name, "  checkout-web {Enter}");
    expect(await within(dialog).findByRole("heading", { name: "When does it expire?" })).toBeVisible();
    expect(within(dialog).getByRole("combobox", { name: "Expires" })).toHaveTextContent("In 90 days");
    expect(within(dialog).getByText("Choose the shortest time the key is needed.")).toBeInTheDocument();
    await next(user, dialog, "What can it call?");
    await next(user, dialog, "Models and limits");
    await user.click(within(dialog).getByRole("button", { name: "Create key" }));

    const reveal = await screen.findByRole("dialog", { name: "Key created" });
    expect(reveal).toBe(dialog);
    expect(within(reveal).queryByRole("heading", { name: "Models and limits" })).not.toBeInTheDocument();
    expect(requests("POST", KEYS).map((r) => r.body)).toEqual([{ name: "checkout-web", access: "inference", expires_in_days: 90 }]);
    expect(within(reveal).getByText("Copy the key now: it's shown once, and Kaeso keeps only a hash of it.")).toBeInTheDocument();
    await user.click(within(reveal).getByRole("button", { name: "Done" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Did you copy the key?" });
    await user.click(within(confirm).getByRole("button", { name: "Go back" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog", { name: "Did you copy the key?" })).not.toBeInTheDocument());
    await user.click(within(screen.getByRole("dialog", { name: "Key created" })).getByRole("button", { name: "Done" }));
    await user.click(within(await screen.findByRole("alertdialog", { name: "Did you copy the key?" })).getByRole("button", { name: "Close without copying" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Key created" })).not.toBeInTheDocument());
  });

  it("opens on the first step with an empty name after a key was shown, without the old key", async () => {
    signedIn();
    projectData({ keys: [apiKey()] });
    on("POST", KEYS, createdKey());
    const { user } = await renderApp(projectPath("keys"));
    await screen.findByRole("gridcell", { name: /checkout-web/ });

    for (const name of ["checkout-web", "nightly-batch"]) {
      await user.click(screen.getByRole("button", { name: "Create key" }));
      const dialog = await screen.findByRole("dialog", { name: /Create key/ });
      expect(within(dialog).getByText("Step 1 of 4")).toBeInTheDocument();
      expect(within(dialog).getByRole("heading", { name: "What is it for?" })).toBeVisible();
      expect(within(dialog).getByRole("textbox", { name: "Name" })).toHaveValue("");
      expect(screen.queryByText(SECRET)).not.toBeInTheDocument();
      await user.type(within(dialog).getByRole("textbox", { name: "Name" }), `${name}{Enter}`);
      await next(user, dialog, "What can it call?");
      await next(user, dialog, "Models and limits");
      await user.click(within(dialog).getByRole("button", { name: "Create key" }));
      const reveal = await screen.findByRole("dialog", { name: "Key created" });
      expect(within(reveal).getByRole("textbox", { name: /^Secret for/ })).toHaveTextContent(SECRET);
      await user.click(within(reveal).getByRole("button", { name: "Done" }));
      await user.click(
        within(await screen.findByRole("alertdialog", { name: "Did you copy the key?" })).getByRole("button", { name: "Close without copying" }),
      );
      await waitFor(() => expect(screen.queryByRole("dialog", { name: "Key created" })).not.toBeInTheDocument());
      expect(screen.queryByText(SECRET)).not.toBeInTheDocument();
    }
    expect(requests("POST", KEYS).map((r) => (r.body as { name: string }).name)).toEqual(["checkout-web", "nightly-batch"]);
  });

  it("opens on the first step again after cancelling half-way", async () => {
    signedIn();
    projectData({ keys: [apiKey()] });
    const { user } = await renderApp(projectPath("keys"));
    await screen.findByRole("gridcell", { name: /checkout-web/ });

    await user.click(screen.getByRole("button", { name: "Create key" }));
    const dialog = await screen.findByRole("dialog", { name: /Create key/ });
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "checkout-web{Enter}");
    await next(user, dialog, "What can it call?");
    await user.click(within(dialog).getByRole("radio", { name: /^Platform/ }));
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: /Create key/ })).not.toBeInTheDocument());

    await user.click(screen.getByRole("button", { name: "Create key" }));
    const again = await screen.findByRole("dialog", { name: /Create key/ });
    expect(within(again).getByRole("heading", { name: "What is it for?" })).toBeVisible();
    expect(within(again).getByRole("textbox", { name: "Name" })).toHaveValue("");
    await user.type(within(again).getByRole("textbox", { name: "Name" }), "web{Enter}");
    await next(user, again, "What can it call?");
    expect(within(again).getByRole("radio", { name: /^Inference(?! and)/ })).toBeChecked();
  });

  it("closes without asking once the key is copied", async () => {
    signedIn();
    projectData({ models: [sonnet] });
    on("POST", KEYS, createdKey());
    const { user, dialog } = await openDialog();

    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "checkout-web{Enter}");
    await next(user, dialog, "What can it call?");
    await next(user, dialog, "Models and limits");
    await user.click(within(dialog).getByRole("button", { name: "Create key" }));
    const reveal = await screen.findByRole("dialog", { name: "Key created" });
    await user.click(within(reveal).getAllByRole("button", { name: /Copy/ })[0] as HTMLElement);
    await user.click(within(reveal).getByRole("button", { name: "Done" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Key created" })).not.toBeInTheDocument());
    expect(screen.queryByRole("alertdialog", { name: "Did you copy the key?" })).not.toBeInTheDocument();
  });

  it("asks before Escape closes a key that wasn't copied, and closes the form with Escape", async () => {
    signedIn();
    projectData();
    on("POST", KEYS, createdKey());
    const { user, dialog } = await openDialog();

    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "checkout-web{Enter}");
    await next(user, dialog, "What can it call?");
    await next(user, dialog, "Models and limits");
    await user.click(within(dialog).getByRole("button", { name: "Create key" }));
    await screen.findByRole("dialog", { name: "Key created" });
    await user.keyboard("{Escape}");
    expect(await screen.findByRole("alertdialog", { name: "Did you copy the key?" })).toBeInTheDocument();
  });

  it("stays open while the key is being created, so its secret is never lost, and shows it when it arrives", async () => {
    signedIn();
    projectData();
    let respond: (key: unknown) => void = () => undefined;
    on("POST", KEYS, () => new Promise((resolve) => (respond = resolve)));
    const { user, dialog } = await openDialog();
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "checkout-web{Enter}");
    await next(user, dialog, "What can it call?");
    await next(user, dialog, "Models and limits");
    await user.click(within(dialog).getByRole("button", { name: "Create key" }));
    await waitFor(() => expect(requests("POST", KEYS)).toHaveLength(1));
    expect(within(dialog).getByRole("button", { name: "Back" })).toBeDisabled();
    expect(within(dialog).getByRole("button", { name: "Close" })).toBeDisabled();
    await user.keyboard("{Escape}");
    expect(screen.getByRole("dialog", { name: /Create key/ })).toBe(dialog);
    respond(createdKey());
    expect(await screen.findByRole("dialog", { name: "Key created" })).toBe(dialog);
    expect(within(dialog).getByText(SECRET)).toBeInTheDocument();
    expect(requests("POST", KEYS)).toHaveLength(1);
  });

  it("closes the form with Escape", async () => {
    signedIn();
    projectData();
    const { user } = await openDialog();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: /Create key/ })).not.toBeInTheDocument());
    expect(requests("POST", KEYS)).toHaveLength(0);
  });

  it("expires on a chosen day within the project's key lifetime", async () => {
    signedIn({ projects: [project({ max_key_lifetime_days: 10 })] });
    projectData();
    on("POST", KEYS, createdKey());
    const { user, dialog } = await openDialog();

    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "short{Enter}");
    await user.click(await within(dialog).findByRole("combobox", { name: "Expires" }));
    await user.click(await screen.findByRole("option", { name: "On a date" }));
    expect(within(dialog).getByRole("button", { name: "Continue" })).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, within(dialog).getByRole("button", { name: "Continue" }), "Pick the day it expires");
    await user.click(within(dialog).getByRole("button", { name: /Expiry date/ }));
    const calendar = await screen.findByRole("dialog", { name: /date/i });
    const enabled = () =>
      within(calendar)
        .queryAllByRole("button")
        .filter((b) => /, \d{4}$/.test(b.getAttribute("aria-label") ?? "") && !b.hasAttribute("disabled"));
    if (enabled().length === 0) await user.click(within(calendar).getByRole("button", { name: /next month/i }));
    expect(
      within(calendar)
        .getAllByRole("button")
        .some((b) => b.hasAttribute("disabled")),
    ).toBe(true);
    await user.click(enabled()[0] as HTMLElement);
    await waitFor(() => expect(within(dialog).getByRole("button", { name: "Continue" })).not.toHaveAttribute("aria-disabled", "true"));
    await next(user, dialog, "What can it call?");
    await next(user, dialog, "Models and limits");
    await user.click(within(dialog).getByRole("button", { name: "Create key" }));

    await screen.findByRole("dialog", { name: "Key created" });
    const body = requests("POST", KEYS)[0]?.body as { expires_on: string };
    expect(body).toEqual({ name: "short", access: "inference", expires_on: expect.stringMatching(/^\d{4}-\d{2}-\d{2}$/) });
    const days = (Date.parse(`${body.expires_on}T00:00:00Z`) - Date.now()) / 86_400_000;
    expect(days).toBeGreaterThan(-1);
    expect(days).toBeLessThanOrEqual(10);
  });

  it("creates a key for a service account with platform access, models and a limit", async () => {
    signedIn();
    projectData({
      accounts: [serviceAccount(), serviceAccount({ id: "sa_old", name: "old-batch", archived_at: "2026-10-01T12:00:00Z" })],
      models: [sonnet, model()],
    });
    on("GET", "orgs/:org/projects/:project/service-accounts/:account/models", access({ all: false, effective: ["cm_sonnet"], models: ["cm_sonnet"] }));
    on("POST", "orgs/:org/projects/:project/service-accounts/:account/keys", createdKey({ name: "deploy" }));
    const { user, dialog } = await openDialog();

    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "deploy");
    const owner = within(dialog).getByRole("combobox", { name: "For" });
    await waitFor(() => expect(owner).not.toBeDisabled());
    expect(within(dialog).getByText("Your key acts with your rights in this project.")).toBeInTheDocument();
    await user.click(owner);
    expect(screen.queryByRole("option", { name: "Service account old-batch" })).not.toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Me (Ada Okafor)" })).toBeInTheDocument();
    await user.click(screen.getByRole("option", { name: "Service account ci-deploy" }));
    expect(within(dialog).getByText("A service account's key acts with the service account's rights.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(await within(dialog).findByRole("combobox", { name: "Expires" }));
    await user.click(await screen.findByRole("option", { name: "Never" }));
    await next(user, dialog, "What can it call?");
    await user.click(within(dialog).getByRole("radio", { name: /Inference and platform/ }));
    await next(user, dialog, "Models and limits");

    await user.click(within(dialog).getByRole("button", { name: "Models" }));
    expect(screen.queryByRole("option", { name: /GPT-5 mini/ })).not.toBeInTheDocument();
    expect(screen.getByRole("listbox", { name: "Models" }).querySelector('[role="img"][aria-label="Anthropic"]')).not.toBeNull();
    await user.click(await screen.findByRole("option", { name: /Sonnet 4.5/ }));
    await user.keyboard("{Escape}");
    await user.click(within(dialog).getByRole("button", { name: "Add limit" }));
    expect(within(dialog).getByRole("combobox", { name: "What it limits" })).toHaveFocus();
    expect(within(dialog).getByRole("heading", { name: "Limit" })).toBeInTheDocument();
    const create = () => within(dialog).getByRole("button", { name: "Create key" });
    expect(create()).toHaveAttribute("aria-disabled", "true");
    await user.type(within(dialog).getByRole("textbox", { name: "Amount" }), "5");
    await waitFor(() => expect(create()).not.toHaveAttribute("aria-disabled", "true"));
    await user.click(create());

    await screen.findByRole("dialog", { name: "Key created" });
    expect(requests("POST", "orgs/:org/projects/:project/service-accounts/:account/keys").map((r) => [r.params.account, r.body])).toEqual([
      [
        "sa_deploy",
        {
          name: "deploy",
          access: "both",
          models: ["cm_sonnet"],
          limits: [{ kind: "spend_budget", amount: "5", period: "month", hard: true, alert_thresholds: [50, 80, 100] }],
        },
      ],
    ]);
  });

  it("keeps only the models the new owner may call when the owner changes", async () => {
    signedIn();
    projectData({ accounts: [serviceAccount()], models: [sonnet, model()] });
    on("GET", "orgs/:org/projects/:project/members/:user/models", access({ effective: ["cm_sonnet", "cm_gpt"] }));
    on("GET", "orgs/:org/projects/:project/service-accounts/:account/models", access({ all: false, effective: ["cm_sonnet"], models: ["cm_sonnet"] }));
    on("POST", "orgs/:org/projects/:project/service-accounts/:account/keys", createdKey({ name: "deploy" }));
    const { user, dialog } = await openDialog();

    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "deploy");
    await waitFor(() => expect(within(dialog).getByRole("combobox", { name: "For" })).not.toBeDisabled());
    await next(user, dialog, "When does it expire?");
    await next(user, dialog, "What can it call?");
    await next(user, dialog, "Models and limits");
    await user.click(within(dialog).getByRole("button", { name: "Models" }));
    await user.click(await screen.findByRole("option", { name: /Sonnet 4.5/ }));
    await user.click(await screen.findByRole("option", { name: /GPT-5 mini/ }));
    await user.keyboard("{Escape}");
    await user.click(within(dialog).getByRole("button", { name: "Add limit" }));
    await user.click(within(dialog).getByRole("button", { name: "Model" }));
    await user.click(await screen.findByRole("option", { name: /GPT-5 mini/ }));
    await user.type(within(dialog).getByRole("textbox", { name: "Amount" }), "5");

    for (let i = 0; i < 3; i++) await user.click(within(dialog).getByRole("button", { name: "Back" }));
    await user.click(within(dialog).getByRole("combobox", { name: "For" }));
    await user.click(await screen.findByRole("option", { name: "Service account ci-deploy" }));
    await next(user, dialog, "When does it expire?");
    await next(user, dialog, "What can it call?");
    await next(user, dialog, "Models and limits");

    await waitFor(() => expect(within(dialog).getByRole("button", { name: "Models" })).not.toHaveTextContent("GPT-5 mini"));
    expect(within(dialog).getByRole("button", { name: "Models" })).toHaveTextContent("Sonnet 4.5");
    expect(within(dialog).getByRole("button", { name: "Model" })).toHaveTextContent("All models");
    await user.click(within(dialog).getByRole("button", { name: "Create key" }));

    await screen.findByRole("dialog", { name: "Key created" });
    expect(requests("POST", "orgs/:org/projects/:project/service-accounts/:account/keys").map((r) => r.body)).toEqual([
      {
        name: "deploy",
        access: "inference",
        expires_in_days: expect.any(Number),
        models: ["cm_sonnet"],
        limits: [{ kind: "spend_budget", amount: "5", period: "month", hard: true, alert_thresholds: [50, 80, 100] }],
      },
    ]);
  });

  it("drops a limit that was added by mistake", async () => {
    signedIn();
    projectData();
    on("POST", KEYS, createdKey());
    const { user, dialog } = await openDialog();

    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "checkout-web{Enter}");
    await next(user, dialog, "What can it call?");
    await next(user, dialog, "Models and limits");
    await user.click(within(dialog).getByRole("button", { name: "Add limit" }));
    expect(within(dialog).getByRole("textbox", { name: "Amount" })).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Remove this limit" }));
    expect(within(dialog).queryByRole("textbox", { name: "Amount" })).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Create key" }));
    await screen.findByRole("dialog", { name: "Key created" });
    expect(requests("POST", KEYS).map((r) => r.body)).toEqual([{ name: "checkout-web", access: "inference", expires_in_days: 90 }]);
  });

  it("keeps each of several limits apart", async () => {
    signedIn();
    projectData();
    on("POST", KEYS, createdKey());
    const { user, dialog } = await openDialog();

    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "checkout-web{Enter}");
    await next(user, dialog, "What can it call?");
    await next(user, dialog, "Models and limits");
    await user.click(within(dialog).getByRole("button", { name: "Add limit" }));
    await user.click(within(dialog).getByRole("button", { name: "Add limit" }));
    const [first, second] = within(dialog).getAllByRole("textbox", { name: "Amount" }) as [HTMLElement, HTMLElement];
    await user.type(first, "5");
    await user.type(second, "7");
    const create = within(dialog).getByRole("button", { name: "Create key" });
    await waitFor(() => expect(create).not.toHaveAttribute("aria-disabled", "true"));
    await user.click(create);
    await screen.findByRole("dialog", { name: "Key created" });
    expect(requests("POST", KEYS).map((r) => (r.body as { limits: { amount: string }[] }).limits.map((l) => l.amount))).toEqual([["5", "7"]]);
  });

  it("keeps to the project's key lifetime", async () => {
    signedIn({ projects: [project({ max_key_lifetime_days: 10 })] });
    projectData();
    const { user, dialog } = await openDialog();

    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "short{Enter}");
    const expires = await within(dialog).findByRole("combobox", { name: "Expires" });
    await waitFor(() => expect(expires).toHaveTextContent("In 7 days"));
    expect(within(dialog).getByText("This project allows keys for up to 10 days.")).toBeInTheDocument();
    await user.click(expires);
    expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual(["In 1 day", "In 7 days", "On a date"]);
  });

  it("falls back to the organization's key lifetime and asks for a day", async () => {
    signedIn();
    on("GET", "orgs/:org", org({ max_key_lifetime_days: 0.5 }));
    projectData();
    const { user, dialog } = await openDialog();

    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "short{Enter}");
    const expires = await within(dialog).findByRole("combobox", { name: "Expires" });
    await waitFor(() => expect(expires).toHaveTextContent("On a date"));
    expect(within(dialog).getByText("Expiry date")).toBeInTheDocument();
    expect(within(dialog).getByText("The key stops working at the start of this day (UTC).")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Continue" })).toHaveAttribute("aria-disabled", "true");
  });

  it("explains why platform access can't be given", async () => {
    signedIn({ projectRights: without("keys.platform_access.grant", "service_accounts.keys.create") });
    projectData({ accounts: [serviceAccount()] });
    const { user, dialog } = await openDialog();

    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "web");
    expect(within(dialog).queryByRole("combobox", { name: "For" })).not.toBeInTheDocument();
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "{Enter}");
    await next(user, dialog, "What can it call?");
    expect(within(dialog).getByRole("radio", { name: /^Platform/ })).toHaveAttribute("aria-disabled", "true");
    expect(within(dialog).getByRole("radio", { name: /Inference and platform/ })).toHaveAttribute("aria-disabled", "true");
    expect(within(dialog).getAllByText("You need Give keys platform access; ask Lin Wei for it.")).toHaveLength(2);
  });

  it("shows the API's message when the key is refused", async () => {
    signedIn();
    projectData();
    on("POST", KEYS, apiError(422, "invalid_request", "A key with this name exists.", { param: "name" }));
    const { user, dialog } = await openDialog();

    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "checkout-web{Enter}");
    await next(user, dialog, "What can it call?");
    await next(user, dialog, "Models and limits");
    await user.click(within(dialog).getByRole("button", { name: "Create key" }));
    expect((await notifications().findAllByText("A key with this name exists.")).length).toBeGreaterThan(0);
    expect(screen.queryByRole("dialog", { name: "Key created" })).not.toBeInTheDocument();
  });
});
