import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import type { SharedLinkView } from "~/api/generated/types";
import { ADMIN_ORG_RIGHTS, ORG, signedIn, withRights } from "~/test/fixtures";
import { member, notifications, paged, security } from "~/test/fixtures/org";
import { apiError, noContent, on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { kindLabel } from "./shared-links-page";

const PATH = `/settings/orgs/${ORG}/shared-links`;
const LINKS = "orgs/:org/shared-links";

const link = (overrides: Partial<SharedLinkView> = {}): SharedLinkView => ({
  id: "shl_1",
  kind: "analytics",
  title: "October spend",
  project: "proj_checkout",
  member: null,
  live: true,
  days: 30,
  from: null,
  to: null,
  metric: "requests",
  show_costs: true,
  hide_members: false,
  hide_keys: false,
  hide_projects: false,
  audience: "anyone",
  created_by: "usr_ada",
  created_at: "2026-10-14T09:00:00Z",
  expires_at: "2026-11-13T09:00:00Z",
  views: 3,
  last_opened_at: "2026-10-15T10:00:00Z",
  ...overrides,
});

function linksApi(list: SharedLinkView[]) {
  let current = list;
  on("GET", "orgs/:org/members", paged([member(), member({ user: { id: "usr_grace", name: "Grace Hopper", email: "grace@example.com" } })]));
  on("GET", LINKS, () => page(current));
  on("DELETE", `${LINKS}/:link`, ({ params }: { params: Record<string, string> }) => {
    current = current.filter((l) => l.id !== params["link"]);
    return noContent();
  });
}

describe("the organization's shared links", () => {
  it("lists the links that still work and revokes one", async () => {
    signedIn();
    linksApi([
      link(),
      link({
        id: "shl_2",
        kind: "member",
        title: "Grace's usage",
        project: null,
        audience: "members",
        created_by: "usr_grace",
        expires_at: null,
        views: 1,
        last_opened_at: null,
      }),
      link({ id: "shl_3", title: "Old project", project: "proj_gone" }),
      link({ id: "shl_4", title: "October snapshot", project: null, live: false, days: null, from: "2026-10-01", to: "2026-10-31" }),
      link({ id: "shl_5", kind: "budget", title: "Checkout budgets", days: null }),
    ]);
    const { user } = await renderApp(PATH);

    const list = await screen.findByRole("grid", { name: "Shared links" });
    await waitFor(() => expect(within(list).getAllByRole("row")).toHaveLength(5));
    const [first, second, third, snapshot, budget] = within(list).getAllByRole("row");
    expect(snapshot).toHaveTextContent("Analytics · 1 Oct 2026 to 31 Oct 2026 · Whole organization");
    expect(budget).toHaveTextContent("Budget status ·");
    expect(budget).not.toHaveTextContent("Last 30 days");
    expect(third).toHaveTextContent("Analytics · Last 30 days · proj_gone");
    expect(first).toHaveTextContent("October spend");
    expect(first).toHaveTextContent("Anyone with the link");
    expect(first).toHaveTextContent("3 views · Expires 13 Nov 2026");
    expect(second).toHaveTextContent("Member stats · Last 30 days · Whole organization · Grace Hopper");
    expect(second).toHaveTextContent("1 view · Never expires");

    await user.click(within(first as HTMLElement).getByRole("button", { name: "Revoke" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Revoke October spend?" });
    expect(dialog).toHaveTextContent("opened 3 times");
    await user.click(within(dialog).getByRole("button", { name: "Revoke" }));
    await waitFor(() => expect(requests("DELETE", `${LINKS}/:link`).map((r) => r.path)).toEqual([`/platform/v1/orgs/${ORG}/shared-links/shl_1`]));
    expect(await notifications().findByText("October spend no longer works")).toBeInTheDocument();
    await waitFor(() => expect(within(list).getAllByRole("row")).toHaveLength(4));
  });

  it("offers to revoke only one's own links without the right to manage them", async () => {
    withRights(ADMIN_ORG_RIGHTS.filter((r) => r !== "usage.share.manage"));
    linksApi([link(), link({ id: "shl_2", title: "Grace's usage", created_by: "usr_grace" })]);
    await renderApp(PATH);
    expect(await screen.findByText("Your read-only links to the organization's usage that still work.")).toBeInTheDocument();
    const list = await screen.findByRole("grid", { name: "Shared links" });
    await waitFor(() => expect(within(list).getAllByRole("button", { name: "Revoke" })).toHaveLength(1));
  });

  it("says when there are none, or they couldn't load", async () => {
    signedIn();
    linksApi([]);
    await renderApp(PATH);
    expect(await screen.findByText("No shared links")).toBeInTheDocument();
  });

  it("says when the list couldn't load", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    on("GET", LINKS, apiError(503, "unavailable", "The gateway failed."));
    await renderApp(PATH);
    expect(await screen.findByText("The shared links couldn't load")).toBeInTheDocument();
  });

  it("names each kind of link", () => {
    expect(["analytics", "member", "budget", "project_card"].map(kindLabel)).toEqual(["Analytics", "Member stats", "Budget status", "Project summary"]);
  });
});

describe("the organization's shared links policy", () => {
  it("chooses who links reach and how long they last", async () => {
    signedIn();
    let current = security({ shared_links: { allow: "members", max_days: 90 } });
    on("GET", "orgs/:org/security", () => current);
    on("PUT", "orgs/:org/security/shared-links", ({ body }: { body: { allow: string; max_days: number | null } }) => {
      current = { ...current, shared_links: body };
      return body;
    });
    const { user } = await renderApp(`/settings/orgs/${ORG}/security`);

    const allow = await screen.findByRole("combobox", { name: "Allow shared links" });
    expect(allow).toHaveTextContent("Members only");
    await user.click(allow);
    await user.click(await screen.findByRole("option", { name: "Anyone with the link" }));
    expect(await notifications().findByText("Shared links saved")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole("combobox", { name: "Allow shared links" })).toHaveTextContent("Anyone with the link"));
    await user.click(screen.getByRole("combobox", { name: "Longest expiry" }));
    await user.click(await screen.findByRole("option", { name: "No limit" }));
    await waitFor(() => expect(requests("PUT", "orgs/:org/security/shared-links")).toHaveLength(2));
    await waitFor(() => expect(screen.getByRole("combobox", { name: "Longest expiry" })).toHaveTextContent("No limit"));
    await user.click(screen.getByRole("combobox", { name: "Longest expiry" }));
    await user.click(await screen.findByRole("option", { name: "After 7 days" }));
    await waitFor(() =>
      expect(requests("PUT", "orgs/:org/security/shared-links").map((r) => r.body)).toEqual([
        { allow: "anyone", max_days: 90 },
        { allow: "anyone", max_days: null },
        { allow: "anyone", max_days: 7 },
      ]),
    );
  });

  it("says who can change it to those who can't", async () => {
    withRights(ADMIN_ORG_RIGHTS.filter((r) => r !== "usage.share.manage"));
    on("GET", "orgs/:org/security", security());
    await renderApp(`/settings/orgs/${ORG}/security`);
    expect(await screen.findByRole("combobox", { name: "Allow shared links" })).toBeDisabled();
    expect(screen.getByText(/Manage shared links/)).toBeInTheDocument();
  });
});
