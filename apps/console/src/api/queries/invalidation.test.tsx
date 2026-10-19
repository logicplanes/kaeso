import { screen, waitFor } from "@testing-library/react";
import type { QueryKey } from "@tanstack/react-query";
import { describe, expect, it } from "vitest";
import { queryClient } from "~/query";
import { ORG, OTHER_PROJECT, PROJECT, org, project, signedIn } from "~/test/fixtures";
import { key, me, modelsDev } from "~/test/fixtures/instance-account";
import { logSettings, notifications, paged } from "~/test/fixtures/org";
import { apiError, on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const LOG_LIST: QueryKey = ["orgs", ORG, "projects", PROJECT, "logs", "list", {}];
const ORG_USAGE: QueryKey = ["orgs", ORG, "usage", { from: "2026-10-01", to: "2026-10-15", groupBy: "day" }];
const ORG_MEMBERS: QueryKey = ["orgs", ORG, "members"];
const PROJECT_MEMBERS: QueryKey = ["orgs", ORG, "projects", PROJECT, "members"];
const PROJECT_VIEW: QueryKey = ["orgs", ORG, "projects", PROJECT, "view"];
const PROJECT_LOG_SETTINGS: QueryKey = ["orgs", ORG, "projects", PROJECT, "logs", "settings"];
const KEYS: QueryKey = ["orgs", ORG, "projects", PROJECT, "keys", "all", {}];
const OTHER_KEYS: QueryKey = ["orgs", ORG, "projects", OTHER_PROJECT, "keys", "all", {}];
const MODELS: QueryKey = ["orgs", ORG, "projects", PROJECT, "models"];
const PLAYGROUND_MODELS: QueryKey = ["orgs", ORG, "projects", PROJECT, "playground", "models"];
const ALIASES: QueryKey = ["orgs", ORG, "projects", PROJECT, "aliases"];

const seed = (...keys: QueryKey[]) => {
  for (const k of keys) queryClient.setQueryData(k, { seeded: true });
};
const invalidated = (k: QueryKey) => queryClient.getQueryState(k)?.isInvalidated;

describe("what a change reloads", () => {
  it("reloads a renamed organization without reloading the lists inside it", async () => {
    signedIn();
    on("PATCH", "orgs/:org", org({ name: "Contoso" }));
    seed(LOG_LIST, ORG_USAGE);
    const { user } = await renderApp(`/settings/orgs/${ORG}/general`);
    const input = await screen.findByRole("textbox", { name: "Organization name" });
    const before = requests("GET", "orgs/:org").length;
    await user.clear(input);
    await user.type(input, "Contoso{Enter}");
    expect(await notifications().findByText("Name saved")).toBeInTheDocument();
    await waitFor(() => expect(requests("GET", "orgs/:org").length).toBeGreaterThan(before));
    expect(invalidated(LOG_LIST)).toBe(false);
    expect(invalidated(ORG_USAGE)).toBe(false);
  });

  it("reloads every project's log settings when the organization's content logging changes", async () => {
    signedIn();
    on("GET", "orgs/:org/logs/settings", logSettings());
    on("PATCH", "orgs/:org/logs/settings", logSettings({ content_logging: false }));
    seed(LOG_LIST, PROJECT_VIEW, PROJECT_LOG_SETTINGS);
    const { user } = await renderApp(`/settings/orgs/${ORG}/logging`);
    await user.click(await screen.findByRole("switch", { name: "Log request content" }));
    expect(await notifications().findByText("Content logging turned off")).toBeInTheDocument();
    expect(invalidated(PROJECT_LOG_SETTINGS)).toBe(true);
    expect(invalidated(PROJECT_VIEW)).toBe(true);
    expect(invalidated(LOG_LIST)).toBe(false);
  });

  it("reloads the member lists that show a new picture, and nothing else of the organizations", async () => {
    signedIn();
    on("GET", "me", me({ avatar_url: "/avatars/ada.webp" }));
    on("DELETE", "me/avatar", me());
    seed(ORG_MEMBERS, PROJECT_MEMBERS, LOG_LIST, ["orgs", ORG, "view"]);
    const { user } = await renderApp("/settings/account/profile");
    await user.click(await screen.findByRole("button", { name: "Remove" }));
    expect(await notifications().findByText("Picture removed")).toBeInTheDocument();
    expect(invalidated(ORG_MEMBERS)).toBe(true);
    expect(invalidated(PROJECT_MEMBERS)).toBe(true);
    expect(invalidated(LOG_LIST)).toBe(false);
    expect(invalidated(["orgs", ORG, "view"])).toBe(false);
  });

  it("reloads only the keys of the project a disabled key belongs to", async () => {
    signedIn();
    on("GET", "me/keys", paged([key()]));
    on("POST", "me/keys/:key/disable", key({ state: "disabled" }));
    seed(KEYS, OTHER_KEYS, LOG_LIST);
    const { user } = await renderApp("/settings/account/keys");
    await user.click(await screen.findByRole("button", { name: "Actions for support-bot" }));
    await user.click(await screen.findByRole("menuitem", { name: "Disable" }));
    expect(await notifications().findByText("support-bot disabled")).toBeInTheDocument();
    expect(invalidated(KEYS)).toBe(true);
    expect(invalidated(OTHER_KEYS)).toBe(false);
    expect(invalidated(LOG_LIST)).toBe(false);
  });

  it("reloads no project's keys when a disabled key's project isn't one of the person's", async () => {
    signedIn();
    on("GET", "me/keys", paged([key({ project_id: "proj_gone" })]));
    on("POST", "me/keys/:key/disable", key({ project_id: "proj_gone", state: "disabled" }));
    seed(KEYS);
    const { user } = await renderApp("/settings/account/keys");
    await user.click(await screen.findByRole("button", { name: "Actions for support-bot" }));
    await user.click(await screen.findByRole("menuitem", { name: "Disable" }));
    expect(await notifications().findByText("support-bot disabled")).toBeInTheDocument();
    expect(invalidated(KEYS)).toBe(false);
  });

  it("reloads the model catalogues after a models.dev sync", async () => {
    signedIn();
    on("GET", "instance/models-dev", modelsDev());
    on("POST", "instance/models-dev/sync", modelsDev({ model_count: 420 }));
    seed(MODELS, PLAYGROUND_MODELS, ALIASES, PROJECT_MEMBERS);
    const { user } = await renderApp("/settings/instance/model-sources");
    await screen.findByText("412 models");
    await user.click(screen.getByRole("button", { name: "Sync now" }));
    expect(await notifications().findByText("Synced: 420 models")).toBeInTheDocument();
    expect(invalidated(MODELS)).toBe(true);
    expect(invalidated(PLAYGROUND_MODELS)).toBe(true);
    expect(invalidated(ALIASES)).toBe(true);
    expect(invalidated(PROJECT_MEMBERS)).toBe(false);
  });

  it("asks again only for what failed when a page that couldn't load is tried again", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project", apiError(400, "invalid_request", "The project id is malformed."));
    seed(LOG_LIST);
    const { user } = await renderApp(`/orgs/${ORG}/projects/${PROJECT}/limits`);
    expect(await screen.findByRole("heading", { name: "Couldn't load this page", level: 1 })).toBeInTheDocument();
    on("GET", "orgs/:org/projects/:project", project());
    on("GET", "orgs/:org/projects/:project/limits", page([]));
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("heading", { name: "Limits", level: 1 })).toBeInTheDocument();
    expect(invalidated(LOG_LIST)).toBe(false);
  });
});
