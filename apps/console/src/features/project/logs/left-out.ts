import { formatBytes, formatNumber } from "@logicplanes/kaeso-ui/format";
import { t } from "~/i18n";

export interface LeftOut {
  mediaType: string;
  bytes: number;
  width?: number;
  height?: number;
  reason: Reason;
}

type Reason = "not_kept" | "too_large";
const NOT_KEPT: Reason = "not_kept";

const isObj = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const count = (v: unknown): number | undefined => (typeof v === "number" && Number.isFinite(v) && v >= 0 ? v : undefined);

export function leftOutOf(v: unknown): LeftOut | null {
  if (!isObj(v) || !isObj(v.kaeso_media)) return null;
  const m = v.kaeso_media;
  const width = count(m.width);
  const height = count(m.height);
  return {
    mediaType: typeof m.media_type === "string" ? m.media_type : "application/octet-stream",
    bytes: count(m.bytes) ?? 0,
    ...(width && height ? { width, height } : {}),
    reason: NOT_KEPT,
  };
}

const NAMES: Record<string, string> = {
  "audio/mpeg": "MP3",
  "audio/mp4": "M4A",
  "audio/x-wav": "WAV",
  "image/svg+xml": "SVG",
  "video/quicktime": "MOV",
  "text/plain": "TXT",
};

export function formatName(mediaType: string): string {
  const type = mediaType.toLowerCase().replace(/;.*$/, "").trim();
  if (NAMES[type]) return NAMES[type];
  if (type === "application/octet-stream" || !type.includes("/")) return "";
  return type
    .slice(type.indexOf("/") + 1)
    .replace(/^(x-|vnd\.)/, "")
    .toUpperCase();
}

export function mediaKind(mediaType: string): "image" | "audio" | "video" | "file" {
  const type = mediaType.toLowerCase();
  if (type.startsWith("image/")) return "image";
  if (type.startsWith("audio/")) return "audio";
  if (type.startsWith("video/")) return "video";
  return "file";
}

export function mediaDetail(m: Pick<LeftOut, "mediaType" | "bytes" | "width" | "height">): string {
  const size = m.width && m.height ? t("logs.media.pixels", "{width} × {height}", { width: formatNumber(m.width), height: formatNumber(m.height) }) : "";
  return [formatName(m.mediaType), size, formatBytes(m.bytes)].filter(Boolean).join(", ");
}

export function leftOutLabel(m: LeftOut, name?: string): string {
  const detail = mediaDetail(m);
  if (name) return t("logs.media.namedNotKept", "{name} ({detail}), not kept", { name, detail });
  switch (mediaKind(m.mediaType)) {
    case "image":
      return t("logs.media.imageNotKept", "Image ({detail}), not kept", { detail });
    case "audio":
      return t("logs.media.audioNotKept", "Audio ({detail}), not kept", { detail });
    case "video":
      return t("logs.media.videoNotKept", "Video ({detail}), not kept", { detail });
    case "file":
      return t("logs.media.fileNotKept", "File ({detail}), not kept", { detail });
  }
}
