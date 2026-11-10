import { formatBytes } from "@logicplanes/kaeso-ui/format";
import { t } from "~/i18n";
import { leftOutOf, mediaKind, type LeftOut } from "./left-out";

export interface Kept {
  mediaType: string;
  base64: string;
  bytes: number;
}

export type LoggedMedia = { kept: Kept } | { leftOut: LeftOut };

type Json = unknown;
const isObj = (v: Json): v is Record<string, Json> => typeof v === "object" && v !== null && !Array.isArray(v);

const DATA_URL = /^data:([^;,]*)(?:;[^;,]*)*;base64,/i;
const BASE64 = /^[A-Za-z0-9+/_-]+={0,2}$/;
const MIN_CHARS = 16;

const decodedBytes = (base64: string) => Math.floor((base64.replace(/=+$/, "").length * 3) / 4);

/** Base64 in the standard alphabet without padding, which is what `atob` and a data: URL take. */
export const standard = (base64: string) => base64.replace(/-/g, "+").replace(/_/g, "/").replace(/=+$/, "");

function decoded(base64: string): Uint8Array | null {
  try {
    return Uint8Array.from(atob(standard(base64)), (c) => c.charCodeAt(0));
  } catch {
    return null;
  }
}

const head = (base64: string): Uint8Array => decoded(base64.slice(0, 32)) ?? new Uint8Array();

// The second byte of an MPEG audio frame's sync word, for MPEG 1, 2 and 2.5, layer III.
const MP3_FRAMES = [0xfb, 0xfa, 0xf3, 0xf2, 0xe3];

// HEIC shares MP4's container, and most browsers neither draw nor play it: it is a download.
const HEIC = ["heic", "heix", "mif1"];

const starts = (bytes: Uint8Array, text: string, at = 0) => [...text].every((c, i) => bytes[at + i] === c.charCodeAt(0));

function sniff(base64: string): string | null {
  const b = head(base64);
  if (starts(b, "\x89PNG")) return "image/png";
  if (b[0] === 0xff && b[1] === 0xd8 && b[2] === 0xff) return "image/jpeg";
  if (starts(b, "GIF8")) return "image/gif";
  if (starts(b, "RIFF")) return starts(b, "WEBP", 8) ? "image/webp" : starts(b, "WAVE", 8) ? "audio/wav" : null;
  if (starts(b, "ID3") || (b[0] === 0xff && MP3_FRAMES.includes(b[1] ?? 0))) return "audio/mpeg";
  if (starts(b, "OggS")) return "audio/ogg";
  if (starts(b, "fLaC")) return "audio/flac";
  if (b[0] === 0x1a && b[1] === 0x45 && b[2] === 0xdf && b[3] === 0xa3) return "video/webm";
  if (starts(b, "ftyp", 4))
    return starts(b, "avif", 8)
      ? "image/avif"
      : HEIC.some((brand) => starts(b, brand, 8))
        ? "image/heic"
        : starts(b, "M4A ", 8)
          ? "audio/mp4"
          : starts(b, "qt  ", 8)
            ? "video/quicktime"
            : "video/mp4";
  if (starts(b, "%PDF-")) return "application/pdf";
  return null;
}

// What the console draws or plays is decided by how the file starts, never by the type a request claimed: an
// SVG or HTML file sent as image/png is a document, and is only ever offered as a download.
const SHOWN: Record<string, "image" | "audio" | "video"> = {
  "image/png": "image",
  "image/jpeg": "image",
  "image/gif": "image",
  "image/webp": "image",
  "image/avif": "image",
  "audio/wav": "audio",
  "audio/mpeg": "audio",
  "audio/ogg": "audio",
  "audio/flac": "audio",
  "audio/mp4": "audio",
  "video/mp4": "video",
  "video/webm": "video",
  "video/quicktime": "video",
};

// The extension a download gets, by what the bytes are; names an app sent never choose it.
const EXTENSIONS: Record<string, string> = {
  "image/png": "png",
  "image/jpeg": "jpg",
  "image/gif": "gif",
  "image/webp": "webp",
  "image/avif": "avif",
  "audio/wav": "wav",
  "audio/mpeg": "mp3",
  "audio/ogg": "ogg",
  "audio/flac": "flac",
  "audio/mp4": "m4a",
  "audio/webm": "webm",
  "video/mp4": "mp4",
  "video/webm": "webm",
  "video/quicktime": "mov",
  "audio/quicktime": "mov",
  "application/pdf": "pdf",
  "image/heic": "heic",
};
const extensionOf = (type: string | null): string => (type && EXTENSIONS[type]) || "bin";
const ALSO: Record<string, string[]> = { jpg: ["jpeg"], ogg: ["oga", "opus"], m4a: ["mp4"], mp4: ["m4v"] };

export interface Shown {
  kind: "image" | "audio" | "video" | "file";
  /** The type the console hands the browser: the one the bytes show, or a plain download. */
  type: string;
  extension: string;
}

export function shownAs(media: Kept): Shown {
  const found = sniff(media.base64);
  const kind = found ? SHOWN[found] : undefined;
  if (!found || !kind) return { kind: "file", type: "application/octet-stream", extension: extensionOf(found) };
  // WebM and MP4 hold audio or video alike; the request's type says which, within the container the bytes show.
  const type = kind === "video" && mediaKind(media.mediaType) === "audio" ? found.replace("video/", "audio/") : found;
  return { kind: type.startsWith("audio/") ? "audio" : kind, type, extension: extensionOf(type) };
}

export function keptOf(value: Json, declared?: string): Kept | null {
  if (typeof value !== "string") return null;
  const url = DATA_URL.exec(value);
  const base64 = url ? value.slice(url[0].length) : value;
  if (!url && declared === undefined) return null;
  if (base64.length < MIN_CHARS || !BASE64.test(base64) || base64.replace(/=+$/, "").length % 4 === 1) return null;
  return { mediaType: (url ? url[1] : declared) || "application/octet-stream", base64, bytes: decodedBytes(base64) };
}

export function mediaOf(value: Json, declared?: string): LoggedMedia | null {
  const leftOut = leftOutOf(value);
  if (leftOut) return { leftOut };
  const kept = keptOf(value, declared);
  return kept ? { kept } : null;
}

const TYPE_KEYS = ["media_type", "mime_type", "mimeType", "mediaType", "content_type", "contentType"];
const FILE_KEYS = new Set(["b64_json", "file_data", "blob", "bytes"]);
const CONTENT_KEYS = new Set(["data", "result", "image", "audio", "video"]);
const OPAQUE_KEYS = new Set(["signature", "encrypted_content", "thoughtSignature", "thought_signature", "id"]);
const ANY_KEY_CHARS = 64;

// The gateway's own reading of where a file sits: under a key that holds bytes, or next to the type it is in.
function fileUnder(key: string, child: Json, declared: string | undefined): Kept | null {
  if (typeof child !== "string" || OPAQUE_KEYS.has(key)) return null;
  if (FILE_KEYS.has(key)) return keptOf(child, declared ?? "");
  if (declared === undefined || TYPE_KEYS.includes(key)) return null;
  return CONTENT_KEYS.has(key) || child.length >= ANY_KEY_CHARS ? keptOf(child, declared) : null;
}

/** Every file in logged JSON, kept or left out, in the order it appears. */
export function collectMedia(value: Json, out: LoggedMedia[] = [], under?: string): LoggedMedia[] {
  if (Array.isArray(value)) for (const item of value) collectMedia(item, out, under);
  else if (isObj(value)) {
    const leftOut = leftOutOf(value);
    if (leftOut) {
      out.push({ leftOut });
      return out;
    }
    const declared = TYPE_KEYS.map((k) => value[k]).find((v): v is string => typeof v === "string");
    const text = under === "text" || value.type === "text";
    for (const [key, child] of Object.entries(value)) {
      const typed = text ? null : fileUnder(key, child, declared);
      if (typed) out.push({ kept: typed });
      else collectMedia(child, out, key);
    }
  } else {
    const kept = keptOf(value);
    if (kept) out.push({ kept });
  }
  return out;
}

const SHORT_AFTER = 512;

/** Logged JSON for reading: a kept file's base64 text gives way to its first characters and its size. */
export function forReading(value: Json): Json {
  if (typeof value === "string") {
    if (value.length <= SHORT_AFTER || !(DATA_URL.test(value) || BASE64.test(value))) return value;
    const from = DATA_URL.exec(value)?.[0].length ?? 0;
    return t("logs.media.shortened", "{start}… ({size})", { start: value.slice(0, from + 24), size: formatBytes(decodedBytes(value.slice(from))) });
  }
  if (Array.isArray(value)) return value.map(forReading);
  if (isObj(value)) return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, forReading(v)]));
  return value;
}

// Characters a name can use to pass for another: controls, format characters such as bidirectional overrides,
// and path separators.
const UNSAFE = /[\p{Cc}\p{Cf}/\\]/gu;

export const cleanName = (name: string | undefined): string =>
  (name ?? "")
    .replace(UNSAFE, "")
    .replace(/^[.\s]+/, "")
    .trim()
    .slice(0, 120);

/** The name a download saves under: the app's name without its extension, and the extension the bytes call for. */
export function fileName(shown: Shown, name: string | undefined, index: number): string {
  const clean = cleanName(name);
  if (!clean) return `${shown.kind}-${index + 1}.${shown.extension}`;
  const dot = clean.lastIndexOf(".");
  const own = dot > 0 ? clean.slice(dot + 1).toLowerCase() : "";
  if (own === shown.extension || ALSO[shown.extension]?.includes(own)) return clean;
  return `${dot > 0 ? clean.slice(0, dot) : clean}.${shown.extension}`;
}

export function toBlob(media: Kept, type: string): Blob {
  const bytes = decoded(media.base64);
  return bytes ? new Blob([bytes as BlobPart], { type }) : new Blob([media.base64], { type: "application/octet-stream" });
}
