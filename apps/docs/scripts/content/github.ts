export const REPOSITORY_API = "https://api.github.com/repos/logicplanes/kaeso";

export type StarCount = { stars: number } | { stars: null; why: string };

export interface StarOptions {
  fetch?: typeof fetch | undefined;
  token?: string | undefined;
  timeoutMs?: number | undefined;
}

const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === "object" && value !== null && !Array.isArray(value);

export async function githubStars({ fetch: get = fetch, token, timeoutMs = 10_000 }: StarOptions = {}): Promise<StarCount> {
  try {
    const res = await get(REPOSITORY_API, {
      headers: {
        accept: "application/vnd.github+json",
        "x-github-api-version": "2022-11-28",
        "user-agent": "kaeso-docs",
        ...(token ? { authorization: `Bearer ${token}` } : {}),
      },
      signal: AbortSignal.timeout(timeoutMs),
    });
    if (!res.ok) return { stars: null, why: `${REPOSITORY_API} answered ${res.status}` };
    const body: unknown = await res.json();
    const stars = isRecord(body) ? body.stargazers_count : undefined;
    if (typeof stars !== "number" || !Number.isSafeInteger(stars) || stars < 0) return { stars: null, why: `${REPOSITORY_API} gave no star count` };
    return { stars };
  } catch (error) {
    return { stars: null, why: `${REPOSITORY_API} can't be reached (${(error as Error).message})` };
  }
}
