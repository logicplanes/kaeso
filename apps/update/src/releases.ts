import type { Locale } from "./i18n";

export const FEED_PATH = "/v1/history.json";
const DOCS = "https://docs.kaeso.app";

export interface Release {
  version: string;
  prerelease: boolean;
  publishedAt: string | null;
  notes: string;
  url: string;
}

const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === "object" && value !== null && !Array.isArray(value);

function readRelease(raw: unknown): Release | null {
  if (!isRecord(raw) || raw.draft === true) return null;
  const { tag_name: tag, html_url: url, published_at: publishedAt, body } = raw;
  if (typeof tag !== "string" || !tag.startsWith("v") || typeof url !== "string") return null;
  return {
    version: tag.slice(1),
    prerelease: raw.prerelease === true,
    publishedAt: typeof publishedAt === "string" ? publishedAt : null,
    notes: typeof body === "string" ? body.trim() : "",
    url,
  };
}

export function readFeed(data: unknown): Release[] {
  if (!Array.isArray(data)) throw new Error("the release feed is not a list");
  return data.flatMap((raw) => readRelease(raw) ?? []);
}

export async function loadReleases(fetchFn: typeof fetch = fetch, signal?: AbortSignal): Promise<Release[]> {
  const res = await fetchFn(FEED_PATH, { headers: { accept: "application/json" }, ...(signal ? { signal } : {}) });
  if (!res.ok) throw new Error(`the release feed answered ${res.status}`);
  return readFeed(await res.json());
}

export function splitReleases(releases: readonly Release[]): { latest: Release | null; others: Release[] } {
  const latest = releases.find((r) => !r.prerelease) ?? releases[0] ?? null;
  return { latest, others: releases.filter((r) => r !== latest) };
}

export function upgradeGuide(locale: Locale): string {
  return `${DOCS}${locale === "en" ? "" : `/${locale}`}/self-hosting/upgrades`;
}
