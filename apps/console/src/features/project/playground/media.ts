import type { PlaygroundParameters } from "@logicplanes/kaeso-ui/playground";
import type { PlaygroundModel } from "~/api/queries/playground";
import { deleteVideo, getVideo, postMedia, videoContent, type MediaAnswer } from "~/api/playground-media";
import { t } from "~/i18n";
import { fileOf, type TurnFile } from "./attachments";
import { usageOf, type ResponseState } from "./responses";

/** Runs one media turn: reports each change, and ends with the finished state. */
export type MediaRun = (signal: AbortSignal, report: (state: ResponseState) => void, start: ResponseState) => Promise<ResponseState>;

export interface MediaContext {
  org: string;
  project: string;
  now: () => number;
}

type Json = Record<string, unknown>;
const obj = (v: unknown): Json | null => (v && typeof v === "object" && !Array.isArray(v) ? (v as Json) : null);
const str = (v: unknown) => (typeof v === "string" ? v : "");

/** How a video job is asked about: the first wait, how each wait grows, and the longest. */
export const videoPolling = { first: 2_000, factor: 1.5, longest: 15_000 };

function wait(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    signal.throwIfAborted();
    const id = setTimeout(() => {
      signal.removeEventListener("abort", abort);
      resolve();
    }, ms);
    const abort = () => {
      clearTimeout(id);
      reject(signal.reason);
    };
    signal.addEventListener("abort", abort, { once: true });
  });
}

// Transcripts billed by length report `{type: "duration"}` usage, which holds no tokens to show.
const tokensOf = (reply: Json) => (typeof obj(reply.usage)?.input_tokens === "number" ? usageOf(reply) : null);

const done = (state: ResponseState, answer: MediaAnswer | null, now: number, patch: Partial<ResponseState>): ResponseState => ({
  ...state,
  status: "completed",
  endedAt: now,
  ...(answer ? { cost: answer.cost, costKnown: answer.costKnown, requestId: answer.requestId } : {}),
  ...patch,
});

async function json(blob: Blob): Promise<Json> {
  try {
    return obj(JSON.parse(await blob.text())) ?? {};
  } catch {
    return {};
  }
}

const IMAGE_SIZES = new Set(["1024x1024", "1536x1024", "1024x1536"]);

function imageFields(m: PlaygroundModel, p: PlaygroundParameters): Record<string, string | number> {
  const o = m.image;
  const count = Math.min(p.imageCount ?? 1, o?.max_count ?? 1);
  return {
    ...(count > 1 ? { n: count } : {}),
    ...(o?.size === "pixels" && p.imageSize && IMAGE_SIZES.has(p.imageSize) ? { size: p.imageSize } : {}),
    ...(p.imageQuality && o?.qualities.includes(p.imageQuality) ? { quality: p.imageQuality } : {}),
  };
}

const form = (fields: Record<string, string | number | File>) => {
  const body = new FormData();
  for (const [name, value] of Object.entries(fields)) body.append(name, value instanceof File ? value : String(value));
  return body;
};

function images(ctx: MediaContext, m: PlaygroundModel, prompt: string, files: readonly TurnFile[], p: PlaygroundParameters): MediaRun {
  return async (signal, _report, start) => {
    const fields = imageFields(m, p);
    const edit = files.length > 0;
    const body = edit
      ? (() => {
          const f = form({ model: m.model, prompt, ...fields });
          for (const file of files) f.append("image[]", fileOf(file));
          return f;
        })()
      : { model: m.model, prompt, ...fields };
    const answer = await postMedia(ctx.org, ctx.project, edit ? "edits" : "images", body, signal);
    const reply = await json(answer.blob);
    const format = str(reply.output_format) || "png";
    const data = Array.isArray(reply.data) ? reply.data.map(obj) : [];
    // The gateway sends a Playground reply's images inline; a link would load from the provider's
    // host, which the console's pages never do.
    const media = data.flatMap((d, i) => {
      const b64 = str(d?.b64_json);
      return b64
        ? [
            {
              src: `data:image/${format};base64,${b64}`,
              name: `image-${i + 1}.${format}`,
              label: t("playground.media.image", "Image {index}", { index: i + 1 }),
              alt: str(d?.revised_prompt) || prompt,
            },
          ]
        : [];
    });
    return done(start, answer, ctx.now(), { media, usage: tokensOf(reply) });
  };
}

function extensionOf(type: string): string {
  const sub = type.split("/")[1]?.split(";")[0]?.trim() ?? "";
  return sub === "mpeg" || sub === "" ? "mp3" : sub;
}

function audio(ctx: MediaContext, m: PlaygroundModel, prompt: string, p: PlaygroundParameters): MediaRun {
  return async (signal, _report, start) => {
    const voice = p.voice && (m.voices.length === 0 || m.voices.includes(p.voice)) ? p.voice : m.voices[0];
    const seconds = m.durations ? p.durationSeconds : undefined;
    const [route, body] =
      m.mode === "speech"
        ? (["speech", { model: m.model, input: prompt, response_format: "mp3", ...(voice ? { voice } : {}) }] as const)
        : m.mode === "music"
          ? (["music", { model_id: m.model, prompt, ...(seconds ? { music_length_ms: Math.round(seconds * 1000) } : {}) }] as const)
          : (["soundEffects", { model_id: m.model, text: prompt, ...(seconds ? { duration_seconds: seconds } : {}) }] as const);
    const answer = await postMedia(ctx.org, ctx.project, route, body, signal);
    const [file, label] =
      m.mode === "speech"
        ? ["speech", t("playground.media.speech", "Speech")]
        : m.mode === "music"
          ? ["music", t("playground.media.music", "Music")]
          : ["sound", t("playground.media.sound", "Sound effect")];
    return done(start, answer, ctx.now(), {
      media: [{ src: URL.createObjectURL(answer.blob), name: `${file}.${extensionOf(answer.blob.type)}`, label }],
    });
  };
}

function transcript(ctx: MediaContext, m: PlaygroundModel, prompt: string, files: readonly TurnFile[], p: PlaygroundParameters): MediaRun {
  return async (signal, _report, start) => {
    const file = files[0];
    if (!file)
      return { ...start, status: "failed", endedAt: ctx.now(), error: t("playground.needsAudio", "Attach or record audio for this model to transcribe.") };
    const body = form({ model: m.model, file: fileOf(file), response_format: "json", ...(prompt ? { prompt } : {}) });
    const answer = await postMedia(ctx.org, ctx.project, p.translate && m.translations ? "translations" : "transcriptions", body, signal);
    const reply = await json(answer.blob);
    return done(start, answer, ctx.now(), { text: str(reply.text), usage: tokensOf(reply) });
  };
}

function video(ctx: MediaContext, m: PlaygroundModel, prompt: string): MediaRun {
  return async (signal, report, start) => {
    const answer = await postMedia(ctx.org, ctx.project, "videos", { model: m.model, prompt }, signal);
    let job = await json(answer.blob);
    const id = str(job.id);
    let state: ResponseState = { ...start, status: "streaming", progress: null, requestId: answer.requestId };
    let delay = videoPolling.first;
    let charge: Pick<MediaAnswer, "cost" | "costKnown"> | null = null;
    try {
      for (;;) {
        const status = str(job.status);
        const progress = typeof job.progress === "number" ? job.progress : null;
        state = { ...state, progress };
        if (status === "completed")
          return done(state, null, ctx.now(), {
            ...(charge ?? {}),
            media: [{ src: videoContent(ctx.org, ctx.project, id), name: `${id}.mp4`, label: t("playground.media.video", "Video") }],
            progress: 100,
          });
        if (status === "failed")
          return {
            ...state,
            status: "failed",
            endedAt: ctx.now(),
            error: str(obj(job.error)?.message) || t("playground.videoFailed", "The provider couldn't make the video. Try again."),
          };
        report(state);
        await wait(delay, signal);
        delay = Math.min(videoPolling.longest, delay * videoPolling.factor);
        const polled = await getVideo(ctx.org, ctx.project, id, signal);
        job = obj(polled.job) ?? {};
        charge = polled.charge;
      }
    } catch (e) {
      if (signal.aborted && id) void deleteVideo(ctx.org, ctx.project, id).catch(() => undefined);
      throw e;
    }
  };
}

export function mediaRun(ctx: MediaContext, m: PlaygroundModel, prompt: string, files: readonly TurnFile[], p: PlaygroundParameters): MediaRun {
  switch (m.mode) {
    case "image":
      return images(ctx, m, prompt, files, p);
    case "transcription":
      return transcript(ctx, m, prompt, files, p);
    case "video":
      return video(ctx, m, prompt);
    default:
      return audio(ctx, m, prompt, p);
  }
}
