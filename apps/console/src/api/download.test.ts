import { afterEach, describe, expect, it, vi } from "vitest";
import { apiError, json, on, requests } from "~/test/network";
import { ApiError } from "./errors";
import { saveExport, type ExportFile } from "./download";

const URL_EXPORT = "/platform/v1/orgs/org_1/projects/proj_1/logs/export?format=csv";
const URL_CHECK = "/platform/v1/orgs/org_1/projects/proj_1/logs?limit=1";
const file: ExportFile = { name: "kaeso-logs-proj_1.csv", description: "CSV", mime: "text/csv", extension: "csv" };

interface Picked {
  chunks: string[];
  closed: boolean;
  aborted: boolean;
  removed: boolean;
}

function installPicker(result: "save" | Error, name = "requests.csv"): { picked: Picked; picker: ReturnType<typeof vi.fn> } {
  const picked: Picked = { chunks: [], closed: false, aborted: false, removed: false };
  const decoder = new TextDecoder();
  const picker = vi.fn(async () => {
    if (result instanceof Error) throw result;
    return {
      name,
      createWritable: async () =>
        new WritableStream<Uint8Array>({
          write(chunk) {
            picked.chunks.push(decoder.decode(chunk));
          },
          close() {
            picked.closed = true;
          },
          abort() {
            picked.aborted = true;
          },
        }),
      remove: async () => {
        picked.removed = true;
      },
    };
  });
  Object.defineProperty(window, "showSaveFilePicker", { value: picker, configurable: true, writable: true });
  return { picked, picker };
}

function streamOf(parts: string[], fail = false): ReadableStream<Uint8Array> {
  const encoder = new TextEncoder();
  return new ReadableStream<Uint8Array>({
    start(c) {
      for (const p of parts) c.enqueue(encoder.encode(p));
      if (fail) c.error(new TypeError("connection reset"));
      else c.close();
    },
  });
}

const options = (controller = new AbortController()) => ({ check: URL_CHECK, signal: controller.signal, onSaving: vi.fn(), onProgress: vi.fn() });

afterEach(() => {
  Reflect.deleteProperty(window, "showSaveFilePicker");
  vi.restoreAllMocks();
});

describe("saveExport without a save picker", () => {
  it("checks the export first, then hands the file to the browser's own download", async () => {
    on("GET", "orgs/:org/projects/:project/logs", json({ data: [], has_more: false }));
    const clicked: HTMLAnchorElement[] = [];
    vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (this: HTMLAnchorElement) {
      clicked.push(this);
    });
    const opts = options();
    await expect(saveExport(URL_EXPORT, file, opts)).resolves.toEqual({ kind: "browser" });
    expect(requests("GET", "orgs/:org/projects/:project/logs")[0]?.query.get("limit")).toBe("1");
    expect(clicked).toHaveLength(1);
    expect(clicked[0]?.getAttribute("href")).toBe(URL_EXPORT);
    expect(clicked[0]?.download).toBe("kaeso-logs-proj_1.csv");
    expect(clicked[0]?.isConnected).toBe(false);
    expect(opts.onSaving).not.toHaveBeenCalled();
  });

  it("says what the gateway refused before any download starts", async () => {
    on("GET", "orgs/:org/projects/:project/logs", apiError(403, "permission_denied", "You need Export logs.", { right: "logs.export" }));
    const click = vi.spyOn(HTMLAnchorElement.prototype, "click");
    const error = await saveExport(URL_EXPORT, file, options()).catch((e: unknown) => e);
    expect(error).toBeInstanceOf(ApiError);
    expect(error).toMatchObject({ message: "You need Export logs.", status: 403, right: "logs.export" });
    expect(click).not.toHaveBeenCalled();
  });

  it("calls it a network failure when the check never reaches the gateway", async () => {
    on("GET", "orgs/:org/projects/:project/logs", () => {
      throw new TypeError("Failed to fetch");
    });
    await expect(saveExport(URL_EXPORT, file, options())).rejects.toMatchObject({ type: "network", message: "Can't reach the gateway; try again." });
  });

  it("is canceled when the person cancels during the check", async () => {
    const controller = new AbortController();
    on("GET", "orgs/:org/projects/:project/logs", () => {
      controller.abort();
      throw new DOMException("The operation was aborted.", "AbortError");
    });
    await expect(saveExport(URL_EXPORT, file, options(controller))).resolves.toEqual({ kind: "canceled" });
  });

  it("falls back to the browser download when the picker fails for another reason", async () => {
    const { picker } = installPicker(new DOMException("Not allowed", "SecurityError"));
    on("GET", "orgs/:org/projects/:project/logs", json({ data: [], has_more: false }));
    vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => undefined);
    await expect(saveExport(URL_EXPORT, file, options())).resolves.toEqual({ kind: "browser" });
    expect(picker).toHaveBeenCalledWith({ suggestedName: "kaeso-logs-proj_1.csv", types: [{ description: "CSV", accept: { "text/csv": [".csv"] } }] });
  });
});

describe("saveExport with a save picker", () => {
  it("does nothing when the person closes the picker", async () => {
    installPicker(new DOMException("The user aborted a request.", "AbortError"));
    const opts = options();
    await expect(saveExport(URL_EXPORT, file, opts)).resolves.toEqual({ kind: "dismissed" });
    expect(requests()).toEqual([]);
    expect(opts.onSaving).not.toHaveBeenCalled();
  });

  it("streams the export into the chosen file and counts the bytes", async () => {
    const { picked } = installPicker("save", "my-requests.csv");
    on(
      "GET",
      "orgs/:org/projects/:project/logs/export",
      () => new Response(streamOf(["id,status\n", "req_1,200\n"]), { headers: { "content-type": "text/csv" } }),
    );
    const opts = options();
    await expect(saveExport(URL_EXPORT, file, opts)).resolves.toEqual({ kind: "saved", bytes: 20, name: "my-requests.csv" });
    expect(picked.chunks.join("")).toBe("id,status\nreq_1,200\n");
    expect(picked.closed).toBe(true);
    expect(opts.onSaving).toHaveBeenCalledTimes(1);
    expect(opts.onProgress.mock.calls).toEqual([[10], [20]]);
    expect(requests("GET", "orgs/:org/projects/:project/logs/export")[0]?.query.get("format")).toBe("csv");
    expect(requests("GET", "orgs/:org/projects/:project/logs")).toEqual([]);
  });

  it("closes an empty file when the export has no body", async () => {
    const { picked } = installPicker("save");
    on("GET", "orgs/:org/projects/:project/logs/export", () => new Response(null, { status: 204 }));
    await expect(saveExport(URL_EXPORT, file, options())).resolves.toEqual({ kind: "saved", bytes: 0, name: "requests.csv" });
    expect(picked.closed).toBe(true);
  });

  it("removes the file and passes on the gateway's error when the export is refused", async () => {
    const { picked } = installPicker("save");
    on("GET", "orgs/:org/projects/:project/logs/export", apiError(422, "invalid_request", "The range is longer than 90 days.", { param: "from" }));
    await expect(saveExport(URL_EXPORT, file, options())).rejects.toMatchObject({ message: "The range is longer than 90 days.", param: "from" });
    expect(picked.aborted).toBe(true);
    expect(picked.removed).toBe(true);
  });

  it("still passes on the gateway's error when the file can't be cleaned up", async () => {
    Object.defineProperty(window, "showSaveFilePicker", {
      configurable: true,
      writable: true,
      value: async () => ({
        name: "requests.csv",
        createWritable: async () =>
          new WritableStream<Uint8Array>({
            abort() {
              throw new Error("disk gone");
            },
          }),
        remove: async () => {
          throw new Error("disk gone");
        },
      }),
    });
    on("GET", "orgs/:org/projects/:project/logs/export", apiError(403, "permission_denied", "You need Export logs."));
    await expect(saveExport(URL_EXPORT, file, options())).rejects.toMatchObject({ message: "You need Export logs.", status: 403 });
  });

  it("removes the half-written file when the stream breaks", async () => {
    const { picked } = installPicker("save");
    on("GET", "orgs/:org/projects/:project/logs/export", () => new Response(streamOf(["id,status\n"], true)));
    await expect(saveExport(URL_EXPORT, file, options())).rejects.toMatchObject({ type: "network", message: "The export stopped before the end; try again." });
    expect(picked.removed).toBe(true);
  });

  it("is canceled, without a file, when the person cancels while it saves", async () => {
    const { picked } = installPicker("save");
    const controller = new AbortController();
    on("GET", "orgs/:org/projects/:project/logs/export", () => {
      controller.abort();
      return new Response(streamOf(["id,status\n"]));
    });
    await expect(saveExport(URL_EXPORT, file, options(controller))).resolves.toEqual({ kind: "canceled" });
    expect(picked.removed).toBe(true);
  });
});
