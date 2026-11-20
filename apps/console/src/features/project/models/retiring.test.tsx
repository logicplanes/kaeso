import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { signedIn } from "~/test/fixtures";
import { hoverReason, notifications } from "~/test/fixtures/org";
import { apiKey, catalogueModel, projectData, projectPath, retiringModel, retiringReplacement, retiringUsage, serviceAccount } from "~/test/fixtures/project";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import type { RetiringModelView } from "~/api/generated/types";

const RETIRING = "orgs/:org/projects/:project/deprecations";
const MOVE = "orgs/:org/projects/:project/deprecations/:model/move";

function setUp(retiring: RetiringModelView[], { soon = 0, rights }: { soon?: number; rights?: string[] } = {}) {
  signedIn(rights ? { projectRights: rights } : {});
  projectData({
    keys: [apiKey({ id: "key_app", name: "checkout-web" })],
    accounts: [serviceAccount({ id: "sa_jobs", name: "nightly-jobs" })],
    models: [
      catalogueModel({ id: "model_gpt4o", provider: "openai", model_id: "gpt-4o", deprecated: true }),
      catalogueModel({ id: "model_old", provider: "openai", model_id: "gpt-3.5-turbo", deprecated: true }),
      catalogueModel({ id: "model_gone", provider: "openai", model_id: "gpt-4", deprecated: true }),
    ],
    retiring,
    soon,
  });
}

describe("retiring models", () => {
  it("warn on the catalogue, mark each row with its shutdown and list them in their own tab", async () => {
    setUp(
      [
        retiringModel(),
        retiringModel({ id: "model_gone", model_id: "gpt-4", state: "retired", days_left: -3, lifecycle: { ...retiringModel().lifecycle, replacement: null } }),
        retiringModel({ id: "model_old", model_id: "gpt-3.5-turbo", days_left: null, lifecycle: { ...retiringModel().lifecycle, shutdown_on: null } }),
      ],
      { soon: 2 },
    );
    const { user } = await renderApp(projectPath("models"));
    expect(await screen.findByText("2 models shut down soon")).toBeInTheDocument();
    expect(screen.getByText("Shuts down 5 Nov 2026")).toBeInTheDocument();
    expect(screen.getByText("Retired")).toBeInTheDocument();
    expect(screen.getAllByText("Deprecated")).toHaveLength(1);
    await user.click(screen.getByRole("button", { name: "Show retiring models" }));
    expect(await screen.findByText("openai/gpt-4o")).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Retiring/ })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText("in 20 days")).toBeInTheDocument();
    expect(screen.getByText("retired 3 days ago")).toBeInTheDocument();
    expect(screen.getAllByText("Replace with gpt-5.5")).toHaveLength(2);
    expect(screen.getByText("No replacement named")).toBeInTheDocument();
    expect(screen.getAllByText("12 requests in 30 days")).toHaveLength(3);
  });

  it("show who still calls a model and move its aliases to the replacement", async () => {
    setUp([retiringModel()]);
    on("GET", MOVE, { model: retiringModel(), adds_replacement: true, steps: 2 });
    on("POST", MOVE, { model: retiringModel({ aliases: [] }), adds_replacement: true, steps: 2 });
    const { user } = await renderApp(projectPath("models?tab=retiring"));
    await user.click(await screen.findByText("openai/gpt-4o"));
    const sheet = await screen.findByRole("dialog", { name: "gpt-4o" });
    expect(within(sheet).getByText("$2.50 → $1.25")).toBeInTheDocument();
    expect(within(sheet).getByText("$10.00 → unknown")).toBeInTheDocument();
    expect(within(sheet).getByRole("link", { name: "chat" })).toHaveAttribute("href", projectPath("models/aliases/alias_chat"));
    expect(within(sheet).getByText("route, route.fallback")).toBeInTheDocument();
    expect(within(sheet).getByRole("link", { name: "example.com (opens in a new tab)" })).toHaveAttribute("href", "https://example.com/deprecations");
    const logs = (name: string) => decodeURIComponent(within(sheet).getByRole("link", { name }).getAttribute("href") ?? "");
    expect(within(sheet).getByRole("heading", { name: "Who still calls it, last 30 days" })).toBeInTheDocument();
    expect(await within(sheet).findByRole("link", { name: "nightly-jobs" })).toBeInTheDocument();
    expect(await within(sheet).findByRole("link", { name: "checkout-web" })).toBeInTheDocument();
    expect(logs("checkout-web")).toContain("key=key_app");
    expect(logs("Ada Okafor (you)")).toContain("member=user:usr_ada");
    expect(logs("nightly-jobs")).toContain("member=service_account:sa_jobs");
    expect(logs("OpenAI Python SDK")).toContain("client=openai-python");
    expect(within(sheet).getByRole("img", { name: "Requests per day since the announcement" })).toBeInTheDocument();

    await user.click(within(sheet).getByRole("button", { name: "Move to replacement" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Move gpt-4o to gpt-5.5?" });
    expect(
      await within(confirm).findByText("2 route steps in chat point at the replacement. The replacement is added to the catalogue with KPMR's prices."),
    ).toBeInTheDocument();
    await user.click(within(confirm).getByRole("button", { name: "Move to replacement" }));
    expect(await notifications().findByText("gpt-4o moved to gpt-5.5")).toBeInTheDocument();
    expect(requests("POST", MOVE)).toHaveLength(1);
  });

  it("say what is missing: no replacement, no alias, no calls, no right to see usage", async () => {
    setUp(
      [
        retiringModel({
          state: "announced",
          days_left: 0,
          lifecycle: { announced_on: null, deprecated_on: null, shutdown_on: "2026-10-15", replacement: null, source: null },
          replacement: null,
          aliases: [],
          usage: { keys: [], members: [], service_accounts: [], clients: [], daily: [], requests: 0 },
        }),
        retiringModel({
          id: "model_old",
          model_id: "gpt-3.5-turbo",
          days_left: null,
          lifecycle: { ...retiringModel().lifecycle, shutdown_on: null },
          usage: null,
        }),
      ],
      { rights: ["models.view"] },
    );
    const { user } = await renderApp(projectPath("models?tab=retiring"));
    expect(await screen.findByText("shuts down today")).toBeInTheDocument();
    expect(screen.getByText("Retirement announced")).toBeInTheDocument();
    await user.click(screen.getByText("openai/gpt-4o"));
    const sheet = await screen.findByRole("dialog", { name: "gpt-4o" });
    expect(within(sheet).getByText("No alias calls it; apps that name it directly still do.")).toBeInTheDocument();
    expect(within(sheet).getByText("No requests in the last 30 days.")).toBeInTheDocument();
    expect(within(sheet).queryByRole("button", { name: "Move to replacement" })).not.toBeInTheDocument();
    await user.keyboard("{Escape}");
    await user.click(screen.getByText("openai/gpt-3.5-turbo"));
    const other = await screen.findByRole("dialog", { name: "gpt-3.5-turbo" });
    expect(within(other).queryByText("Who still calls it, last 30 days")).not.toBeInTheDocument();
    expect(within(other).queryByRole("button", { name: "Move to replacement" })).not.toBeInTheDocument();
  });

  it("list only the callers it has, unlinked without the request log, and can't move to a replacement Kaeso doesn't know", async () => {
    setUp(
      [
        retiringModel({
          replacement: retiringReplacement({ listed: false }),
          usage: retiringUsage({ members: [], service_accounts: [], clients: [] }),
        }),
      ],
      { rights: ["models.view", "models.manage"] },
    );
    const { user } = await renderApp(projectPath("models?tab=retiring"));
    await user.click(await screen.findByText("openai/gpt-4o"));
    const sheet = await screen.findByRole("dialog", { name: "gpt-4o" });
    expect(within(sheet).getByText("key_app")).not.toHaveAttribute("href");
    expect(within(sheet).queryByText("Members")).not.toBeInTheDocument();
    expect(within(sheet).queryByText("Client apps")).not.toBeInTheDocument();
    expect(within(sheet).getByRole("button", { name: "Move to replacement" })).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, within(sheet).getByRole("button", { name: "Move to replacement" }), "KPMR doesn't list the replacement, so it can't be added.");
  });

  it("say why there's nothing to move when no alias calls the model", async () => {
    setUp([retiringModel({ aliases: [] })], { rights: ["models.view", "models.manage"] });
    const { user } = await renderApp(projectPath("models?tab=retiring"));
    await user.click(await screen.findByText("openai/gpt-4o"));
    const sheet = await screen.findByRole("dialog", { name: "gpt-4o" });
    const move = within(sheet).getByRole("button", { name: "Move to replacement" });
    expect(move).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, move, "Nothing to move; no alias calls it.");
  });

  it("move to a replacement already in the catalogue", async () => {
    setUp([retiringModel({ replacement: retiringReplacement({ catalogue_model_id: "model_gpt55" }) })]);
    on("GET", MOVE, { model: retiringModel(), adds_replacement: false, steps: 1 });
    on("POST", MOVE, { model: retiringModel({ aliases: [] }), adds_replacement: false, steps: 1 });
    const { user } = await renderApp(projectPath("models?tab=retiring"));
    await user.click(await screen.findByText("openai/gpt-4o"));
    const sheet = await screen.findByRole("dialog", { name: "gpt-4o" });
    await user.click(within(sheet).getByRole("button", { name: "Move to replacement" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Move gpt-4o to gpt-5.5?" });
    expect(await within(confirm).findByText("1 route step in chat points at the replacement.")).toBeInTheDocument();
    await user.click(within(confirm).getByRole("button", { name: "Move to replacement" }));
    expect(await notifications().findByText("gpt-4o moved to gpt-5.5")).toBeInTheDocument();
  });

  it("show an empty tab and a failed load", async () => {
    setUp([]);
    await renderApp(projectPath("models?tab=retiring"));
    expect(await screen.findByText("No model of this project is retiring")).toBeInTheDocument();
  });

  it("say when the list doesn't load", async () => {
    setUp([]);
    on("GET", RETIRING, apiError(500, "internal_error", "Down."));
    await renderApp(projectPath("models?tab=retiring"));
    expect(await screen.findByText("The retiring models couldn't load")).toBeInTheDocument();
  });
});
