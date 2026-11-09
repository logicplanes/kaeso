import type { ImageParameterOptions, ParameterPanelProps, PlaygroundParameters, ReasoningEffort } from "@logicplanes/kaeso-ui/playground";
import type { PlaygroundModel } from "~/api/queries/playground";
import { EFFORTS } from "~/format/playground-search";
import { t } from "~/i18n";
import { filesFor, type TurnFile } from "./attachments";
import { mediaRun, type MediaContext } from "./media";
import { requestFor } from "./request";
import type { Mode, Turn } from "./responses";
import type { Send } from "./use-threads";

export function modeLabel(mode: Mode): string {
  switch (mode) {
    case "image":
      return t("playground.mode.image", "Images");
    case "speech":
      return t("playground.mode.speech", "Speech");
    case "sound_effects":
      return t("playground.mode.soundEffects", "Sound effects");
    case "music":
      return t("playground.mode.music", "Music");
    case "transcription":
      return t("playground.mode.transcription", "Transcription");
    case "video":
      return t("playground.mode.video", "Video");
    default:
      return "";
  }
}

/** What one column sends for a prompt, in its model's mode. */
export function sendFor(
  ctx: MediaContext,
  m: PlaygroundModel,
  turns: readonly Turn[],
  prompt: string,
  files: readonly TurnFile[],
  p: PlaygroundParameters,
): Omit<Send, "column"> {
  const taken = filesFor(m, files);
  return m.mode === "text"
    ? { mode: "text", files: taken, request: requestFor(m, turns, prompt, p, taken) }
    : { mode: m.mode, files: taken, media: mediaRun(ctx, m, prompt, taken, p) };
}

type PanelOptions = Pick<ParameterPanelProps, "instructions" | "sampling" | "reasoning" | "text" | "image" | "voices" | "duration" | "translate">;

/** The parameters the chosen models take, each section only for the modes they cover. */
export function panelOptions(chosen: readonly PlaygroundModel[]): PanelOptions {
  const texts = chosen.filter((m) => m.mode === "text");
  const images = chosen.flatMap((m) => (m.mode === "image" && m.image ? [m.image] : []));
  const speakers = chosen.filter((m) => m.mode === "speech");
  const reasoners = texts.filter((m) => m.reasoning);
  const efforts: ReasoningEffort[] | boolean = reasoners.length
    ? reasoners.some((m) => m.efforts.length === 0)
      ? true
      : EFFORTS.filter((e) => reasoners.some((m) => m.efforts.includes(e)))
    : false;
  const image: ImageParameterOptions | undefined = images.length
    ? {
        maxCount: Math.max(...images.map((o) => o.max_count)),
        qualities: [...new Set(images.flatMap((o) => o.qualities))],
        sizes: images.some((o) => o.size === "pixels"),
      }
    : undefined;
  const voices = speakers.length ? (speakers.every((m) => m.voices.length) ? [...new Set(speakers.flatMap((m) => m.voices))] : true) : false;
  const text = chosen.length === 0 || texts.length > 0;
  return {
    text,
    instructions: text,
    sampling: chosen.length === 0 || texts.some((m) => m.temperature),
    reasoning: efforts,
    ...(image ? { image } : {}),
    voices,
    duration: chosen.some((m) => m.mode === "sound_effects" || m.mode === "music"),
    translate: chosen.some((m) => m.mode === "transcription" && m.translations),
  };
}

/** How many images an image model makes for the parameters, so its column holds room for them. */
export const imagesExpected = (m: PlaygroundModel | null, p: PlaygroundParameters) =>
  m?.mode === "image" ? Math.max(1, Math.min(p.imageCount ?? 1, m.image?.max_count ?? 1)) : 1;
