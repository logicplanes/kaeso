import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { RouteStep } from "~/api/generated/types";
import { signedIn } from "~/test/fixtures";
import { alias, apiKey, catalogueModel, modelRoute, modelsApi, projectData, projectPath } from "~/test/fixtures/project";
import { on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const ONE_ALIAS = "orgs/:org/projects/:project/aliases/:alias";
const TRACE = "orgs/:org/projects/:project/aliases/trace";

type Sent = {
  request: {
    headers: { name: string; value: string }[];
    format: string;
    features: string[];
    input_tokens: number;
    key: string | null;
    service_account: string | null;
  };
};

const models = () => [
  catalogueModel(),
  catalogueModel({ id: "cm_haiku", model_id: "claude-haiku-4-5", effective: { ...catalogueModel().effective, display_name: "Claude Haiku 4.5" } }),
];

const route: RouteStep = {
  type: "condition",
  match: "any",
  rules: [
    { field: "header", name: "x-plan", op: "equals", value: "enterprise" },
    { field: "input_tokens", op: "at_least", value: 100000 },
  ],
  then: modelRoute("cm_sonnet"),
  else: {
    type: "split",
    by: "header",
    header: "x-user-id",
    branches: [
      { weight: 30, step: modelRoute("cm_sonnet") },
      { weight: 70, step: modelRoute("cm_haiku") },
    ],
  },
};

const last = () => requests("POST", TRACE).at(-1)?.body as Sent;

async function open() {
  signedIn();
  const one = alias({ route });
  projectData({
    aliases: [one],
    models: models(),
    keys: [apiKey({ id: "key_batch", name: "nightly-batch", principal: { kind: "service_account", id: "sa_batch" } })],
  });
  modelsApi({ models: models() });
  on("GET", ONE_ALIAS, one);
  on("POST", TRACE, ({ body }: { body: Sent }) =>
    body.request.input_tokens >= 100000
      ? {
          steps: [
            { type: "condition", path: "route", holds: true, rules: [false, true] },
            { type: "model", path: "route.then", catalogue_model_id: "cm_sonnet", attempt: 1 },
          ],
        }
      : {
          steps: [
            { type: "condition", path: "route", holds: false, rules: [false, false] },
            { type: "split", path: "route.else", branch: 0, weight: 30, draw: 12 },
            { type: "model", path: "route.else.branches.0.step", catalogue_model_id: "cm_sonnet", attempt: 1 },
          ],
        },
  );
  const app = await renderApp(projectPath("models/aliases/al_fast"));
  await app.user.click(await screen.findByRole("tab", { name: "Test" }));
  return { ...app, panel: await screen.findByRole("complementary", { name: "Step settings" }) };
}

describe("TestPanel", () => {
  it("offers every header the route reads and sends each fact of the request", async () => {
    const { user, panel } = await open();

    expect(within(panel).getByRole("textbox", { name: "Name of header 1" })).toHaveValue("x-plan");
    expect(within(panel).getByRole("textbox", { name: "Name of header 2" })).toHaveValue("x-user-id");
    expect(await within(panel).findByText("0 of 2 rules hold; takes Else")).toBeInTheDocument();
    expect(within(panel).getByText("Drew 12 of 100; takes branch 1, 30%")).toBeInTheDocument();

    await user.click(within(panel).getByRole("combobox", { name: "API format" }));
    await user.click(await screen.findByRole("option", { name: "Anthropic Messages" }));
    await waitFor(() => expect(last().request.format).toBe("anthropic_messages"));

    await user.click(within(panel).getByRole("combobox", { name: "Uses" }));
    await user.click(await screen.findByRole("option", { name: "Images" }));
    await user.keyboard("{Escape}");
    await waitFor(() => expect(last().request.features).toEqual(["images"]));

    await user.click(within(panel).getByRole("combobox", { name: "API key" }));
    await user.click(await screen.findByRole("option", { name: /nightly-batch/ }));
    await waitFor(() => expect(last().request).toMatchObject({ key: "key_batch", service_account: "sa_batch" }));
    await user.click(within(panel).getByRole("combobox", { name: "API key" }));
    await user.click(await screen.findByRole("option", { name: "No key" }));
    expect(within(panel).getByRole("combobox", { name: "API key" })).toHaveTextContent("No key");

    const tokens = within(panel).getByRole("textbox", { name: "Input tokens" });
    await user.clear(tokens);
    expect(tokens).toHaveValue("");
    await user.type(tokens, "150k000");
    expect(tokens).toHaveValue("150000");
    expect(await within(panel).findByText("1 of 2 rules hold; takes Then")).toBeInTheDocument();
    expect(last().request.input_tokens).toBe(150000);
  });

  it("stays on the trace when the background is clicked, and opens the alias when the start is", async () => {
    const { user, panel } = await open();
    await within(panel).findByText("0 of 2 rules hold; takes Else");

    fireEvent.pointerDown(document.querySelector("[data-flow-canvas]") as HTMLElement, { button: 0, pointerId: 1, pointerType: "mouse" });
    fireEvent.pointerUp(document.querySelector("[data-flow-canvas]") as HTMLElement, { button: 0, pointerId: 1, pointerType: "mouse" });
    expect(screen.getByRole("tab", { name: "Test" })).toHaveAttribute("aria-selected", "true");

    await user.click(screen.getByRole("treeitem", { name: /^Start/ }));
    expect(await screen.findByRole("heading", { name: "Alias" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Editor" })).toHaveAttribute("aria-selected", "true");
  });
});
