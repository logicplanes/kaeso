import { describe, expect, it, vi } from "vitest";
import { REPOSITORY_API, githubStars } from "./github.ts";

const answer = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status });

describe("the star count at build time", () => {
  it("comes from GitHub's repository API, with the token when there is one", async () => {
    const get = vi.fn<typeof fetch>(async () => answer({ full_name: "logicplanes/kaeso", stargazers_count: 1234 }));
    expect(await githubStars({ fetch: get, token: "ghp_example" })).toEqual({ stars: 1234 });
    const [url, init] = get.mock.calls[0] ?? [];
    expect(url).toBe("https://api.github.com/repos/logicplanes/kaeso");
    expect(init?.headers).toMatchObject({ accept: "application/vnd.github+json", authorization: "Bearer ghp_example" });
    expect(init?.signal).toBeInstanceOf(AbortSignal);
    await githubStars({ fetch: get });
    expect(get.mock.calls[1]?.[1]?.headers).not.toHaveProperty("authorization");
  });

  it("is missing, with the reason, when GitHub refuses, can't be reached or gives no count", async () => {
    expect(await githubStars({ fetch: async () => answer({ message: "API rate limit exceeded" }, 403) })).toEqual({
      stars: null,
      why: `${REPOSITORY_API} answered 403`,
    });
    expect(
      await githubStars({
        fetch: async () => {
          throw new TypeError("fetch failed");
        },
      }),
    ).toEqual({ stars: null, why: `${REPOSITORY_API} can't be reached (fetch failed)` });
    for (const body of [[], { stargazers_count: "4" }, { stargazers_count: -1 }, { stargazers_count: 1.5 }])
      expect(await githubStars({ fetch: async () => answer(body) })).toEqual({ stars: null, why: `${REPOSITORY_API} gave no star count` });
    expect(await githubStars({ fetch: async () => new Response("<html>", { status: 200 }) })).toMatchObject({ stars: null });
  });

  it("gives up when GitHub takes too long", async () => {
    const hanging: typeof fetch = (_url, init) =>
      new Promise((_resolve, reject) => init?.signal?.addEventListener("abort", () => reject(init.signal?.reason as Error)));
    expect(await githubStars({ fetch: hanging, timeoutMs: 5 })).toMatchObject({ stars: null, why: expect.stringContaining("can't be reached") });
  });
});
