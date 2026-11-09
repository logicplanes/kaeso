import { api } from "./client";

const BASE = "/platform/v1/orgs/{org}/projects/{project}/playground";

export const MEDIA_ROUTES = {
  images: `${BASE}/images/generations`,
  edits: `${BASE}/images/edits`,
  speech: `${BASE}/audio/speech`,
  transcriptions: `${BASE}/audio/transcriptions`,
  translations: `${BASE}/audio/translations`,
  soundEffects: `${BASE}/sound-effects`,
  music: `${BASE}/music`,
  videos: `${BASE}/videos`,
} as const;

export type MediaRoute = keyof typeof MEDIA_ROUTES;

export interface MediaAnswer {
  blob: Blob;
  /** The amount the request was charged, `null` when the gateway said nothing about it. */
  cost: string | null;
  costKnown: boolean;
  requestId: string | null;
}

type Loose = (path: string, init: object) => Promise<{ data?: Blob; response: Response }>;

export async function postMedia(
  org: string,
  project: string,
  route: MediaRoute,
  body: Record<string, unknown> | FormData,
  signal: AbortSignal,
): Promise<MediaAnswer> {
  const post = api.POST as unknown as Loose;
  const { data, response } = await post(MEDIA_ROUTES[route], {
    params: { path: { org, project } },
    body,
    ...(body instanceof FormData ? { bodySerializer: (b: FormData) => b } : {}),
    parseAs: "blob",
    signal,
  });
  const cost = response.headers.get("kaeso-cost");
  return {
    blob: data ?? new Blob([]),
    cost: cost && cost !== "unknown" ? cost : null,
    costKnown: cost !== "unknown",
    requestId: response.headers.get("kaeso-request-id"),
  };
}

type Video = (path: string, init: object) => Promise<{ data?: unknown }>;

export async function getVideo(org: string, project: string, video: string, signal: AbortSignal): Promise<unknown> {
  const get = api.GET as unknown as Video;
  return (await get(`${BASE}/videos/{video}`, { params: { path: { org, project, video } }, signal })).data;
}

export async function deleteVideo(org: string, project: string, video: string): Promise<void> {
  const del = api.DELETE as unknown as Video;
  await del(`${BASE}/videos/{video}`, { params: { path: { org, project, video } } });
}

export const videoContent = (org: string, project: string, video: string) =>
  `/platform/v1/orgs/${encodeURIComponent(org)}/projects/${encodeURIComponent(project)}/playground/videos/${encodeURIComponent(video)}/content`;
