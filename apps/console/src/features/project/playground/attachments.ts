import type { PlaygroundModel } from "~/api/queries/playground";
import type { Attachment } from "@logicplanes/kaeso-ui/playground";

export interface TurnFile {
  id: string;
  name: string;
  type: string;
  data: string;
}

// The image types every provider's vision input takes: OpenAI, Anthropic and Gemini all read these.
const IMAGES = ["image/png", "image/jpeg", "image/webp", "image/gif"];
const PDF = "application/pdf";
const AUDIO = "audio/*";

// The prompt's text, the history and the request's own fields ride along with the files.
const ROOM_FOR_TEXT = 64 * 1024;

// Browsers record the microphone as WebM or MP4, which some name a video container.
const audio = (type: string) => type.startsWith("audio/") || type === "video/webm" || type === "video/mp4";

/** The file types a model reads from the composer, by its mode. */
export function typesOf(m: PlaygroundModel): string[] {
  switch (m.mode) {
    case "text":
      return [...(m.input_modalities.includes("image") ? IMAGES : []), ...(m.input_modalities.includes("pdf") ? [PDF] : [])];
    case "image":
      return m.image?.edits ? IMAGES : [];
    case "transcription":
      return [AUDIO];
    default:
      return [];
  }
}

const reads = (types: readonly string[], type: string) => types.some((t) => t === type || (t === AUDIO && audio(type)));

/** The files of a prompt that go to a model: what it reads, and for a transcription only the first recording. */
export function filesFor(m: PlaygroundModel, files: readonly TurnFile[]): TurnFile[] {
  const types = typesOf(m);
  const taken = files.filter((f) => reads(types, f.type));
  return m.mode === "transcription" ? taken.slice(0, 1) : taken;
}

export interface FileRules {
  types: string[];
  takes: (type: string) => boolean;
  maxBytes: (type: string) => number;
}

export function fileRules(models: readonly PlaygroundModel[]): FileRules {
  const types = [...new Set(models.flatMap(typesOf))];
  return {
    types,
    takes: (type) => reads(types, type),
    maxBytes: (type) => {
      const limits = models.filter((m) => reads(typesOf(m), type)).map((m) => m.max_request_bytes);
      return limits.length ? Math.max(0, Math.min(...limits) - ROOM_FOR_TEXT) : 0;
    },
  };
}

export const acceptOf = (rules: FileRules) => (rules.types.length ? rules.types.join(",") : undefined);

export const sizeOf = (files: readonly TurnFile[]) => files.reduce((n, f) => n + f.data.length, 0);

export function readFile(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error ?? new Error("unreadable"));
    reader.readAsDataURL(file);
  });
}

export function fileOf(file: TurnFile): File {
  const [head = "", body = ""] = file.data.split(",", 2);
  const bytes = Uint8Array.from(atob(body), (c) => c.charCodeAt(0));
  return new File([bytes], file.name, { type: /^data:([^;,]+)/.exec(head)?.[1] ?? file.type });
}

let next = 0;
export const fileId = () => `file${++next}`;

export const shown = (files: readonly TurnFile[] | undefined): Attachment[] =>
  (files ?? []).map((f) => ({ id: f.id, name: f.name, type: f.type, ...(f.type.startsWith("image/") ? { src: f.data } : {}) }));

type Part = { type: "input_text"; text: string } | { type: "input_image"; image_url: string } | { type: "input_file"; filename: string; file_data: string };

export function userContent(prompt: string, files: readonly TurnFile[] | undefined): string | Part[] {
  if (!files?.length) return prompt;
  return [
    ...(prompt ? [{ type: "input_text" as const, text: prompt }] : []),
    ...files.map((f): Part =>
      f.type.startsWith("image/") ? { type: "input_image", image_url: f.data } : { type: "input_file", filename: f.name, file_data: f.data },
    ),
  ];
}
