import { t } from "~/i18n";
import { ApiError, errorFromResponse, networkError } from "./errors";

export interface ExportFile {
  name: string;
  description: string;
  mime: string;
  extension: string;
}

export type ExportOutcome = { kind: "saved"; bytes: number; name: string } | { kind: "browser" } | { kind: "dismissed" } | { kind: "canceled" };

interface SaveFileHandle {
  readonly name: string;
  createWritable(): Promise<WritableStream<Uint8Array>>;
  remove?(): Promise<void>;
}
type SaveFilePicker = (options: { suggestedName: string; types: { description: string; accept: Record<string, string[]> }[] }) => Promise<SaveFileHandle>;

export async function saveExport(
  url: string,
  file: ExportFile,
  { check, signal, onSaving, onProgress }: { check: string; signal: AbortSignal; onSaving: () => void; onProgress: (bytes: number) => void },
): Promise<ExportOutcome> {
  const picker = (window as { showSaveFilePicker?: SaveFilePicker }).showSaveFilePicker;
  let handle: SaveFileHandle | undefined;
  if (picker) {
    try {
      handle = await picker({ suggestedName: file.name, types: [{ description: file.description, accept: { [file.mime]: [`.${file.extension}`] } }] });
    } catch (e) {
      if (e instanceof DOMException && e.name === "AbortError") return { kind: "dismissed" };
    }
  }
  if (!handle) {
    try {
      await (await get(check, signal)).body?.cancel();
    } catch (e) {
      if (signal.aborted) return { kind: "canceled" };
      throw e;
    }
    const a = document.createElement("a");
    a.href = url;
    a.download = file.name;
    document.body.append(a);
    a.click();
    a.remove();
    return { kind: "browser" };
  }

  const saved = handle;
  let bytes = 0;
  const discard = () => saved.remove?.().catch(() => undefined);
  try {
    const writable = await saved.createWritable();
    onSaving();
    let response: Response;
    try {
      response = await get(url, signal);
    } catch (e) {
      await writable.abort().catch(() => {});
      throw e;
    }
    const counted = new TransformStream<Uint8Array, Uint8Array>({
      transform(chunk, out) {
        bytes += chunk.byteLength;
        onProgress(bytes);
        out.enqueue(chunk);
      },
    });
    if (response.body) await response.body.pipeThrough(counted).pipeTo(writable, { signal });
    else await writable.close();
  } catch (e) {
    await discard();
    if (signal.aborted) return { kind: "canceled" };
    if (e instanceof ApiError) throw e;
    throw new ApiError({ type: "network", status: 0, message: t("errors.exportStopped", "The export stopped before the end. Try again.") });
  }
  return { kind: "saved", bytes, name: saved.name };
}

async function get(url: string, signal: AbortSignal): Promise<Response> {
  let response: Response;
  try {
    response = await fetch(url, { credentials: "same-origin", signal });
  } catch (e) {
    if (signal.aborted) throw e;
    throw networkError();
  }
  if (!response.ok) throw await errorFromResponse(response);
  return response;
}
