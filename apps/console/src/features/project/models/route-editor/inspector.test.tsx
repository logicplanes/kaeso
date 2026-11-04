import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { RouteStep } from "~/api/generated/types";
import { USER, signedIn } from "~/test/fixtures";
import { alias, apiKey, catalogueModel, me, modelRoute, modelsApi, projectData, projectPath, serviceAccount } from "~/test/fixtures/project";
import { on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const ONE_ALIAS = "orgs/:org/projects/:project/aliases/:alias";

const models = () => [
  catalogueModel(),
  catalogueModel({ id: "cm_haiku", model_id: "claude-haiku-4-5", effective: { ...catalogueModel().effective, display_name: "Claude Haiku 4.5" } }),
];

const routed = (route: RouteStep) =>
  alias({
    route,
    models: [
      { catalogue_model_id: "cm_sonnet", target: "anthropic/claude-sonnet-4-5" },
      { catalogue_model_id: "cm_haiku", target: "anthropic/claude-haiku-4-5" },
    ],
  });

const conditionRoute: RouteStep = {
  type: "condition",
  match: "all",
  rules: [{ field: "header", name: "x-plan", op: "equals", value: "paid" }],
  then: modelRoute("cm_sonnet"),
  else: modelRoute("cm_haiku"),
};

const splitRoute: RouteStep = {
  type: "split",
  by: "request",
  branches: [
    { weight: 80, step: modelRoute("cm_sonnet") },
    { weight: 20, step: modelRoute("cm_haiku") },
  ],
};

async function open(route: RouteStep) {
  signedIn();
  projectData({
    aliases: [routed(route)],
    models: models(),
    keys: [apiKey()],
    members: [me()],
    accounts: [serviceAccount()],
  });
  modelsApi({ models: models() });
  on("GET", ONE_ALIAS, routed(route));
  on("PATCH", ONE_ALIAS, ({ body }: { body: { route?: RouteStep } }) => routed(body.route ?? route));
  return renderApp(projectPath("models/aliases/al_fast"));
}

const panel = () => screen.getByRole("complementary", { name: "Step settings" });
const savedRoute = () => (requests("PATCH", ONE_ALIAS).at(-1)?.body as { route: RouteStep }).route;
const KIND = "^(?:Model|Condition|Split|Start)";
const step = (name: RegExp) => screen.findByRole("treeitem", { name: new RegExp(`${KIND}.*${name.source}`) });

async function choose(user: Awaited<ReturnType<typeof open>>["user"], control: HTMLElement, option: RegExp | string) {
  await user.click(control);
  await user.click(await screen.findByRole("option", { name: option }));
}

describe("route inspector", () => {
  it("edits a header rule, hides the value for presence, and combines rules", async () => {
    const { user } = await open(conditionRoute);
    await user.click(await step(/x-plan is paid/));
    const p = panel();
    const value = within(p).getByRole("textbox", { name: "Header value" });
    await user.clear(value);
    await user.type(value, "pro");
    expect(await step(/x-plan is pro/)).toBeInTheDocument();
    await choose(user, within(p).getByRole("combobox", { name: "Comparison" }), "Is sent");
    expect(within(p).queryByRole("textbox", { name: "Header value" })).not.toBeInTheDocument();
    expect(await step(/x-plan is sent/)).toBeInTheDocument();
    const name = within(p).getByRole("textbox", { name: "Header name" });
    await user.clear(name);
    await user.type(name, "Authorization");
    expect(await step(/authorization is sent/)).toHaveTextContent("Header carries credentials");
    await user.clear(name);
    await user.type(name, "x-team");

    await choose(user, within(p).getByRole("combobox", { name: "Comparison" }), "Is");
    expect(within(p).getByRole("textbox", { name: "Header value" })).toHaveValue("");
    await user.type(within(p).getByRole("textbox", { name: "Header value" }), "pro");
    await choose(user, within(p).getByRole("combobox", { name: "Comparison" }), "Contains");
    expect(within(p).getByRole("textbox", { name: "Header value" })).toHaveValue("pro");
    await choose(user, within(p).getByRole("combobox", { name: "Comparison" }), "Is sent");

    await user.click(within(p).getByRole("button", { name: "Add rule" }));
    expect(within(p).getByRole("radiogroup", { name: "How rules combine" })).toBeInTheDocument();
    await user.click(within(p).getByRole("radio", { name: "Any rule holds" }));
    await user.click(within(p).getByRole("radio", { name: "All rules hold" }));
    await user.click(within(p).getByRole("radio", { name: "Any rule holds" }));
    const second = within(p).getByRole("region", { name: "Rule 2" });
    await choose(user, within(second).getByRole("combobox", { name: "What the rule reads" }), "Feature");
    await choose(user, within(second).getByRole("combobox", { name: "What the rule reads" }), "Header");
    await user.type(within(second).getByRole("textbox", { name: "Header name" }), "x-b");
    await user.click(within(p).getByRole("button", { name: "Remove rule 2" }));
    expect(within(p).queryByRole("radiogroup", { name: "How rules combine" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(requests("PATCH", ONE_ALIAS)).toHaveLength(1));
    expect(savedRoute()).toMatchObject({ type: "condition", match: "any", rules: [{ field: "header", name: "x-team", op: "present", value: null }] });
  });

  it("reads formats, features and token counts", async () => {
    const { user } = await open(conditionRoute);
    await user.click(await step(/x-plan is paid/));
    const p = panel();
    const field = () => within(p).getByRole("combobox", { name: "What the rule reads" });

    await choose(user, field(), "API format");
    expect(await step(/Sent as Anthropic Messages/)).toBeInTheDocument();
    await choose(user, within(p).getByRole("combobox", { name: "Comparison" }), "Is none of");
    await user.click(within(p).getByRole("combobox", { name: "API formats" }));
    await user.click(await screen.findByRole("option", { name: "Anthropic Messages" }));
    await user.keyboard("{Escape}");
    expect(await step(/Not sent as/)).toHaveTextContent("Choose a format");

    await choose(user, field(), "Feature");
    expect(await step(/Uses Images/)).toBeInTheDocument();
    await choose(user, within(p).getByRole("combobox", { name: "Comparison" }), "Doesn't use");
    await choose(user, within(p).getByRole("combobox", { name: "Feature" }), "Tools");
    expect(await step(/Doesn't use Tools/)).toBeInTheDocument();

    await choose(user, field(), "Input tokens");
    const tokens = within(p).getByRole("textbox", { name: "Input tokens" });
    await user.clear(tokens);
    expect(await step(/0 input tokens or more/)).toHaveTextContent("Give a token count");
    await user.type(tokens, "2k000");
    await choose(user, within(p).getByRole("combobox", { name: "Comparison" }), "Fewer than");
    expect(await step(/Under 2,000 input tokens/)).toBeInTheDocument();
  });

  it("picks keys, members and service accounts by name", async () => {
    const { user } = await open(conditionRoute);
    await user.click(await step(/x-plan is paid/));
    const p = panel();
    const field = () => within(p).getByRole("combobox", { name: "What the rule reads" });
    for (const [kind, label, option, shown] of [
      ["API key", "API key", /checkout-web/, /Sent with checkout-web/],
      ["Member", "Member", /Ada Okafor/, /Sent by Ada Okafor/],
      ["Service account", "Service account", /ci-deploy/, /Sent by ci-deploy/],
    ] as const) {
      await choose(user, field(), kind);
      expect(await step(/Sent (with|by)/)).toHaveTextContent("Choose at least one");
      await user.click(within(p).getByRole("combobox", { name: label }));
      await user.click(await screen.findByRole("option", { name: option }));
      await user.keyboard("{Escape}");
      expect(await step(shown)).toBeInTheDocument();
    }
    await choose(user, within(p).getByRole("combobox", { name: "Comparison" }), "Is none of");
    expect(await step(/Not sent by ci-deploy/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(requests("PATCH", ONE_ALIAS)).toHaveLength(1));
    expect(savedRoute()).toMatchObject({ rules: [{ field: "service_account", op: "not_in", value: ["sa_deploy"] }] });
    expect(USER).toBeTruthy();
  });

  it("removes a condition and keeps one side", async () => {
    const { user } = await open(conditionRoute);
    await user.click(await step(/x-plan is paid/));
    await user.click(within(panel()).getByRole("button", { name: "Remove" }));
    await user.click(await screen.findByRole("menuitem", { name: "Remove, keep Else" }));
    await waitFor(() => expect(screen.queryByRole("treeitem", { name: /^Condition/ })).not.toBeInTheDocument());
    expect(await step(/Claude Haiku 4\.5/)).toBeInTheDocument();
    expect(screen.queryByRole("treeitem", { name: /^Model.*Claude Sonnet 4\.5/ })).not.toBeInTheDocument();
  });

  it("weighs a split's branches, adds and removes them, and shares evenly", async () => {
    const { user } = await open(splitRoute);
    await user.click(await step(/Per request/));
    const p = panel();
    await user.click(within(p).getByRole("radio", { name: "Per caller" }));
    expect(within(p).getByText("Each key, member or service account stays on one branch, for A/B tests.")).toBeInTheDocument();
    await user.click(within(p).getByRole("radio", { name: "Per request" }));
    await user.click(within(p).getByRole("radio", { name: "Per caller" }));
    const first = within(p).getByRole("textbox", { name: /Branch 1: Claude Sonnet 4\.5/ });
    await user.clear(first);
    await user.type(first, "70");
    expect(within(p).getByText("90%, not 100%")).toBeInTheDocument();
    expect(await step(/By caller/)).toHaveTextContent("Weights add up to 90%");
    await user.click(within(p).getByRole("button", { name: "Share evenly" }));
    expect(within(p).getByText("100%")).toBeInTheDocument();
    await user.click(within(p).getByRole("button", { name: "Add branch" }));
    const weights = () =>
      within(panel())
        .getAllByRole("textbox", { name: /Branch/ })
        .map((i) => (i as HTMLInputElement).value);
    await waitFor(() => expect(weights()).toEqual(["34", "33", "33"]));
    await user.click(await step(/No model yet/));
    await user.click(within(panel()).getByRole("button", { name: "Remove this branch" }));
    await user.click(await step(/By caller/));
    await user.click(within(panel()).getByRole("button", { name: "Add branch" }));
    await waitFor(() => expect(weights()).toEqual(["34", "33", "33"]));
    await user.click(within(panel()).getByRole("button", { name: "Remove branch 3" }));
    await waitFor(() => expect(weights()).toEqual(["67", "33"]));
    await user.click(within(panel()).getByRole("button", { name: "Remove" }));
    await user.click(await screen.findByRole("menuitem", { name: "Remove, keep the 33% branch" }));
    await waitFor(() => expect(screen.queryByRole("treeitem", { name: /^Split/ })).not.toBeInTheDocument());
  });

  it("sets a step's timeout and warns about a fallback it never reaches", async () => {
    const { user } = await open(modelRoute("cm_sonnet", modelRoute("cm_haiku")));
    await user.click(await step(/Claude Sonnet 4\.5/));
    const p = panel();
    const timeout = within(p).getByRole("textbox", { name: /Timeout in seconds/ });
    await user.type(timeout, "9000");
    expect(within(p).getByText("Enter 1 to 3600 seconds.")).toBeInTheDocument();
    await user.clear(timeout);
    await user.type(timeout, "30");
    for (const name of [/The provider is unavailable/, /A rate limit or budget is reached/, /The model can't take the request/])
      await user.click(within(p).getByRole("checkbox", { name }));
    expect(await step(/Claude Sonnet 4\.5/)).toHaveTextContent("Choose when it falls back");
    expect(within(p).getByText("Choose when it falls back")).toBeInTheDocument();
    await user.click(within(p).getByRole("checkbox", { name: /The provider is unavailable/ }));
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(requests("PATCH", ONE_ALIAS)).toHaveLength(1));
    expect(savedRoute()).toMatchObject({ timeout_seconds: 30, fall_back_on: ["unavailable"] });
  });

  it("adds a fallback from the panel and warns when the name is a model's", async () => {
    const { user } = await open(modelRoute("cm_sonnet"));
    await user.click(await step(/Claude Sonnet 4\.5/));
    await user.click(within(panel()).getByRole("button", { name: "Add a fallback model" }));
    expect(await step(/No model yet/)).toHaveAttribute("aria-selected", "true");
    await user.click(screen.getByRole("treeitem", { name: /^Start/ }));
    const name = within(panel()).getByRole("textbox", { name: "Name" });
    await user.clear(name);
    await user.type(name, "claude-sonnet-4-5");
    expect(within(panel()).getByText("Apps that send `claude-sonnet-4-5` get this alias, not the model with the same name.")).toBeInTheDocument();
    expect(within(panel()).getByText("1 step needs a fix before you can save.")).toBeInTheDocument();
    expect(within(panel()).getByRole("tab", { name: "Python" })).toBeInTheDocument();
  });

  it("names removed models and callers it can't find", async () => {
    const route: RouteStep = {
      type: "condition",
      match: "all",
      rules: [
        { field: "key", op: "in", value: ["key_gone"] },
        { field: "member", op: "in", value: ["usr_gone"] },
        { field: "service_account", op: "in", value: ["sa_gone"] },
      ],
      then: modelRoute("cm_gone"),
      else: modelRoute("cm_haiku"),
    };
    await open(route);
    expect(await step(/Sent with key_gone/)).toHaveTextContent("All of 3 rules");
    expect(await step(/Removed model/)).toHaveTextContent("Model removed");
  });
});
