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

// The prompt's text, the history and the request's own fields ride along with the files.
const ROOM_FOR_TEXT = 64 * 1024;

export interface FileRules {
  types: string[];
  maxBytes: number;
}

export function fileRules(models: readonly PlaygroundModel[]): FileRules {
  const all = (modality: string) => models.length > 0 && models.every((m) => m.input_modalities.includes(modality));
  return {
    types: [...(all("image") ? IMAGES : []), ...(all("pdf") ? [PDF] : [])],
    maxBytes: models.length ? Math.max(0, Math.min(...models.map((m) => m.max_request_bytes)) - ROOM_FOR_TEXT) : 0,
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

let next = 0;
export const fileId = () => `file${++next}`;

export const shown = (files: readonly TurnFile[] | undefined): Attachment[] =>
  (files ?? []).map((f) => ({ id: f.id, name: f.name, type: f.type, src: f.data }));

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
