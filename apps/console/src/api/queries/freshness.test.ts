import { focusManager, InfiniteQueryObserver, QueryObserver, type InfiniteData, type QueryKey } from "@tanstack/react-query";
import { waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { queryClient } from "~/query";
import { sessionQuery } from "~/auth/session-queries";
import { ORG, PROJECT, signedIn } from "~/test/fixtures";
import { instanceOrg, instanceUser } from "~/test/fixtures/instance-account";
import { logEntry, logRow } from "~/test/fixtures/project-logs";
import { auditEvent } from "~/test/fixtures/shell";
import { on, page, requests, type ApiRequest } from "~/test/network";
import { auditLogQuery } from "./audit";
import { instanceOrgsQuery, instanceUsersQuery } from "./instance";
import { logEntryQuery, logsInfiniteQuery } from "./logs";

const refocus = () => {
  focusManager.setFocused(true);
  queryClient.getQueryCache().onFocus();
};

afterEach(() => focusManager.setFocused(undefined));

const twoPages =
  <T>(first: T, second: T) =>
  ({ query }: ApiRequest) =>
    query.get("after") ? page([second]) : page([first], "cur_2");

const lists = [
  {
    name: "request log",
    path: "orgs/:org/projects/:project/logs",
    reply: twoPages(logRow({ id: "req_1" }), logRow({ id: "req_2" })),
    options: () => logsInfiniteQuery(ORG, PROJECT, {}),
  },
  {
    name: "audit log",
    path: "orgs/:org/audit-log",
    reply: twoPages(auditEvent({ id: "evt_1" }), auditEvent({ id: "evt_2" })),
    options: () => auditLogQuery({ org: ORG }, {}),
  },
  {
    name: "instance's users",
    path: "instance/users",
    reply: twoPages(instanceUser({ id: "usr_1" }), instanceUser({ id: "usr_2" })),
    options: () => instanceUsersQuery("", undefined),
  },
  {
    name: "instance's organizations",
    path: "instance/orgs",
    reply: twoPages(instanceOrg({ id: "org_1" }), instanceOrg({ id: "org_2" })),
    options: () => instanceOrgsQuery(""),
  },
];

describe("a long list when the window regains focus", () => {
  it.each(lists)("refreshes the $name while one page is loaded, and leaves loaded pages alone after that", async ({ path, reply, options }) => {
    on("GET", path, reply);
    const observer = new InfiniteQueryObserver<unknown, Error, InfiniteData<unknown>, QueryKey, unknown>(queryClient, {
      ...(options() as object),
      staleTime: 0,
    } as never);
    const stop = observer.subscribe(() => undefined);
    await waitFor(() => expect(requests("GET", path)).toHaveLength(1));

    refocus();
    await waitFor(() => expect(requests("GET", path)).toHaveLength(2));

    await observer.fetchNextPage();
    expect(requests("GET", path)).toHaveLength(3);
    refocus();
    expect(queryClient.isFetching()).toBe(0);
    expect(requests("GET", path)).toHaveLength(3);
    stop();
  });
});

describe("how long data stays fresh", () => {
  it("keeps a request's details, which don't change, for minutes", async () => {
    on("GET", "orgs/:org/projects/:project/logs/:log", logEntry());
    await queryClient.fetchQuery(logEntryQuery(ORG, PROJECT, "req_0001"));
    queryClient
      .getQueryCache()
      .find({ queryKey: logEntryQuery(ORG, PROJECT, "req_0001").queryKey })
      ?.setState({ dataUpdatedAt: Date.now() - 60_000 });
    await queryClient.prefetchQuery(logEntryQuery(ORG, PROJECT, "req_0001"));
    expect(requests("GET", "orgs/:org/projects/:project/logs/:log")).toHaveLength(1);
  });

  it("shares a just-checked session between the parts of a page, and checks it again on focus", async () => {
    signedIn();
    await queryClient.fetchQuery(sessionQuery);
    const observers = [new QueryObserver(queryClient, sessionQuery), new QueryObserver(queryClient, sessionQuery)];
    const stops = observers.map((o) => o.subscribe(() => undefined));
    expect(queryClient.isFetching()).toBe(0);
    expect(requests("GET", "auth/session")).toHaveLength(1);

    queryClient
      .getQueryCache()
      .find({ queryKey: sessionQuery.queryKey })
      ?.setState({ dataUpdatedAt: Date.now() - 6_000 });
    refocus();
    await waitFor(() => expect(requests("GET", "auth/session")).toHaveLength(2));
    for (const stop of stops) stop();
  });
});
