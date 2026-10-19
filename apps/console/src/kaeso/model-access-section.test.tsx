import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ALL_PROJECT_RIGHTS, signedIn } from "~/test/fixtures";
import { notifications } from "~/test/fixtures/org";
import { access, apiKey, catalogueModel, memberRights, projectData, projectMember, projectPath, serviceAccount } from "~/test/fixtures/project";
import { model } from "~/test/fixtures/shell";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const sonnet = catalogueModel({ effective: { ...catalogueModel().effective, display_name: "Sonnet 4.5" } });

const MEMBER = "orgs/:org/projects/:project/members/:user";
const MODELS = [sonnet, model()];

async function memberSheet(memberAccess = access({ effective: ["cm_sonnet", "cm_gpt"] }), rights: readonly string[] = ALL_PROJECT_RIGHTS) {
  signedIn({ projectRights: rights });
  projectData({ members: [projectMember()], models: MODELS });
  on("GET", MEMBER, projectMember());
  on("GET", `${MEMBER}/rights`, memberRights(["project.view"]));
  on("GET", `${MEMBER}/models`, memberAccess);
  const view = await renderApp(projectPath("members?member=usr_grace"));
  const sheet = await screen.findByRole("dialog", { name: "Grace Hopper" });
  await within(sheet).findByRole("heading", { name: "Models" });
  return { ...view, sheet };
}

describe("ModelAccessSection", () => {
  it("says how many catalogue models a member may call", async () => {
    const { sheet } = await memberSheet();
    expect(await within(sheet).findByText("Every model in the catalogue: 2 models.")).toBeInTheDocument();
  });

  it("names the models a member is narrowed to", async () => {
    const { sheet } = await memberSheet(access({ all: false, models: ["cm_sonnet", "cm_retired"], effective: ["cm_sonnet"] }));
    expect(await within(sheet).findByText("Sonnet 4.5, cm_retired")).toBeInTheDocument();
  });

  it("says when a member may call no model", async () => {
    const { sheet } = await memberSheet(access({ all: false, models: [], effective: [] }));
    expect(await within(sheet).findByText("None")).toBeInTheDocument();
  });

  it("narrows a member to chosen models", async () => {
    on("PATCH", `${MEMBER}/models`, access({ all: false, models: ["cm_gpt"], effective: ["cm_gpt"] }));
    const { user, sheet } = await memberSheet();

    await within(sheet).findByText("Every model in the catalogue: 2 models.");
    await user.click(within(sheet).getByRole("button", { name: "Edit" }));
    expect(within(sheet).getByText("Their keys' model lists are narrowed to match.")).toBeInTheDocument();
    const picker = within(sheet).getByRole("button", { name: "Models" });
    expect(picker).toHaveTextContent("Every model in the catalogue");
    await user.click(picker);
    await user.click(await screen.findByRole("option", { name: /GPT-5 mini/ }));
    await user.keyboard("{Escape}");
    await user.click(within(sheet).getByRole("button", { name: "Save" }));

    expect(await notifications().findByText("Models saved")).toBeInTheDocument();
    expect(requests("PATCH", `${MEMBER}/models`).map((r) => r.body)).toEqual([{ models: ["cm_gpt"] }]);
    await waitFor(() => expect(within(sheet).queryByRole("button", { name: "Models" })).not.toBeInTheDocument());
  });

  it("opens every model again when the list is emptied", async () => {
    on("PATCH", `${MEMBER}/models`, access({ effective: ["cm_sonnet", "cm_gpt"] }));
    const { user, sheet } = await memberSheet(access({ all: false, models: ["cm_sonnet"], effective: ["cm_sonnet"] }));

    await within(sheet).findByText("Sonnet 4.5");
    await user.click(within(sheet).getByRole("button", { name: "Edit" }));
    await user.click(within(sheet).getByRole("button", { name: "Models" }));
    const chosen = await screen.findByRole("option", { name: /Sonnet 4.5/ });
    expect(chosen).toHaveAttribute("aria-selected", "true");
    await user.click(chosen);
    await user.keyboard("{Escape}");
    await user.click(within(sheet).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(requests("PATCH", `${MEMBER}/models`).map((r) => r.body)).toEqual([{ all: true }]));
  });

  it("discards the edit on cancel and keeps it when saving fails", async () => {
    on("PATCH", `${MEMBER}/models`, apiError(422, "invalid_request", "That model isn't in the catalogue."));
    const { user, sheet } = await memberSheet();

    await within(sheet).findByText("Every model in the catalogue: 2 models.");
    await user.click(within(sheet).getByRole("button", { name: "Edit" }));
    const models = within(sheet).getByRole("heading", { name: "Models" }).closest("section") as HTMLElement;
    await user.click(within(models).getByRole("button", { name: "Cancel" }));
    expect(within(sheet).getByText("Every model in the catalogue: 2 models.")).toBeInTheDocument();
    await user.click(within(sheet).getByRole("button", { name: "Edit" }));
    await user.click(within(sheet).getByRole("button", { name: "Save" }));
    expect((await notifications().findAllByText("That model isn't in the catalogue.")).length).toBeGreaterThan(0);
    expect(within(sheet).getByRole("button", { name: "Models" })).toBeInTheDocument();
  });

  it("can't be edited without Manage limits", async () => {
    const { sheet } = await memberSheet(
      undefined,
      ALL_PROJECT_RIGHTS.filter((r) => r !== "limits.manage"),
    );
    expect(await within(sheet).findByText("Every model in the catalogue: 2 models.")).toBeInTheDocument();
    expect(within(sheet).queryByRole("button", { name: "Edit" })).not.toBeInTheDocument();
  });

  it("narrows a service account", async () => {
    signedIn();
    projectData({ accounts: [serviceAccount()], models: MODELS });
    on("GET", "orgs/:org/projects/:project/service-accounts/:account", serviceAccount());
    on("GET", "orgs/:org/projects/:project/service-accounts/:account/models", access({ effective: ["cm_sonnet"] }));
    on("PATCH", "orgs/:org/projects/:project/service-accounts/:account/models", access({ all: false, models: ["cm_sonnet"], effective: ["cm_sonnet"] }));
    const { user } = await renderApp(projectPath("service-accounts?account=sa_deploy"));

    const sheet = await screen.findByRole("dialog", { name: "ci-deploy" });
    expect(await within(sheet).findByText("Every model in the catalogue: 1 model.")).toBeInTheDocument();
    await user.click(within(sheet).getByRole("button", { name: "Edit" }));
    await user.click(within(sheet).getByRole("button", { name: "Models" }));
    await user.click(await screen.findByRole("option", { name: /Sonnet 4.5/ }));
    await user.keyboard("{Escape}");
    await user.click(within(sheet).getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(requests("PATCH", "orgs/:org/projects/:project/service-accounts/:account/models").map((r) => [r.params.account, r.body])).toEqual([
        ["sa_deploy", { models: ["cm_sonnet"] }],
      ]),
    );
  });

  it("offers a key only the models its owner may use", async () => {
    signedIn();
    projectData({ keys: [apiKey()], models: MODELS });
    on("GET", "orgs/:org/projects/:project/keys/:key", apiKey());
    on("GET", "orgs/:org/projects/:project/keys/:key/models", access({ effective: ["cm_gpt"] }));
    on("GET", `${MEMBER}/models`, access({ all: false, models: ["cm_gpt", "cm_custom"], effective: ["cm_gpt", "cm_custom"] }));
    on("PATCH", "orgs/:org/projects/:project/keys/:key/models", access({ all: false, models: ["cm_custom"], effective: ["cm_custom"] }));
    const { user } = await renderApp(projectPath("keys?key=key_web"));

    const sheet = await screen.findByRole("dialog", { name: /checkout-web/ });
    expect(await within(sheet).findByText("Every model its owner may use: 1 model.")).toBeInTheDocument();
    await user.click(within(sheet).getByRole("button", { name: "Edit" }));
    expect(within(sheet).queryByText("Their keys' model lists are narrowed to match.")).not.toBeInTheDocument();
    const picker = within(sheet).getByRole("button", { name: "Models" });
    expect(picker).toHaveTextContent("Every model its owner may use");
    await user.click(picker);
    expect(await screen.findByRole("option", { name: /GPT-5 mini/ })).toBeInTheDocument();
    expect(screen.queryByRole("option", { name: /Sonnet 4.5/ })).not.toBeInTheDocument();
    await user.click(screen.getByRole("option", { name: /cm_custom/ }));
    await user.keyboard("{Escape}");
    await user.click(within(sheet).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(requests("PATCH", "orgs/:org/projects/:project/keys/:key/models").map((r) => r.body)).toEqual([{ models: ["cm_custom"] }]));
  });
});
