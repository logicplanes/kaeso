import { describe, expect, it, vi } from "vitest";
import { createAppRouter, loadPageChunks } from "./router";

function loadedFor(pathname: string) {
  const router = createAppRouter();
  const load = vi.spyOn(router, "loadRouteChunk");
  return { done: loadPageChunks(router, pathname), routes: () => load.mock.calls.map(([route]) => route.id) };
}

describe("loading a page's code at start", () => {
  it("asks for the code of every route the sign-in page is made of", async () => {
    const { done, routes } = loadedFor("/sign-in");
    expect(routes()).toEqual(["__root__", "/sign-in"]);
    await expect(done).resolves.toBeDefined();
  });

  it("asks for the frame and the project's overview together", () => {
    const { routes } = loadedFor("/orgs/northwind/projects/support-bot");
    expect(routes()).toEqual(["__root__", "/_app", "/_app/orgs/$org", "/_app/orgs/$org/projects/$project", "/_app/orgs/$org/projects/$project/"]);
  });

  it("settles even when a route's code can't be fetched", async () => {
    const router = createAppRouter();
    vi.spyOn(router, "loadRouteChunk").mockRejectedValue(new Error("offline"));
    await expect(loadPageChunks(router, "/sign-in")).resolves.toEqual([
      { status: "rejected", reason: new Error("offline") },
      { status: "rejected", reason: new Error("offline") },
    ]);
  });
});
