import { waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { notifications } from "~/test/fixtures/org";
import { apiError, json, on, requests } from "~/test/network";
import { renderUi } from "~/test/render";
import { showExportProgress, startExport } from "./export-job";

const EXPORT = "orgs/:org/projects/:project/logs/export";
const CHECK = "orgs/:org/projects/:project/logs";
const job = {
  title: (size: string | null) => (size ? `Exporting request logs, ${size}` : "Exporting request logs"),
  done: "Request logs exported",
  url: "/platform/v1/orgs/org_northwind/projects/proj_checkout/logs/export?format=csv",
  check: "/platform/v1/orgs/org_northwind/projects/proj_checkout/logs?limit=1",
  file: { name: "request-logs.csv", description: "CSV", mime: "text/csv", extension: "csv" },
};

function installPicker(pick: () => Promise<unknown> = async () => undefined) {
  const written: Uint8Array[] = [];
  const picker = vi.fn(async () => {
    await pick();
    return {
      name: "request-logs.csv",
      createWritable: async () =>
        new WritableStream<Uint8Array>({
          write(chunk) {
            written.push(chunk);
          },
        }),
      remove: async () => undefined,
    };
  });
  Object.defineProperty(window, "showSaveFilePicker", { value: picker, configurable: true, writable: true });
  return written;
}

function streamedExport() {
  let controller: ReadableStreamDefaultController<Uint8Array> | undefined;
  const body = new ReadableStream<Uint8Array>({
    start(c) {
      controller = c;
    },
  });
  on("GET", EXPORT, () => new Response(body, { status: 200, headers: { "content-type": "text/csv" } }));
  return {
    push: (bytes: number) => controller?.enqueue(new Uint8Array(bytes)),
    end: () => controller?.close(),
  };
}

function leaving(): boolean {
  const event = new Event("beforeunload", { cancelable: true });
  window.dispatchEvent(event);
  return event.defaultPrevented;
}

const finished = () => waitFor(() => expect(showExportProgress()).toBe(false));

afterEach(async () => {
  await finished();
  Reflect.deleteProperty(window, "showSaveFilePicker");
  vi.restoreAllMocks();
});

describe("an export", () => {
  it("shows its progress while saving, warns before leaving and says when the file is saved", async () => {
    await renderUi(<></>);
    const written = installPicker();
    const stream = streamedExport();
    startExport(job);
    expect(await notifications().findAllByText("Exporting request logs")).not.toHaveLength(0);
    expect(leaving()).toBe(true);
    stream.push(2048);
    await waitFor(() => expect(written).toHaveLength(1));
    expect(showExportProgress()).toBe(true);
    expect(await notifications().findAllByText("Exporting request logs, 2.0 KB")).not.toHaveLength(0);
    startExport(job);
    expect(requests("GET", EXPORT)).toHaveLength(1);
    stream.end();
    expect(await notifications().findByText("Request logs exported")).toBeInTheDocument();
    expect(notifications().getByText(/^2\.0 KB in /)).toBeInTheDocument();
    await waitFor(() => expect(notifications().queryByText(/^Exporting request logs/)).not.toBeInTheDocument());
    expect(written.reduce((n, c) => n + c.byteLength, 0)).toBe(2048);
    await finished();
    expect(leaving()).toBe(false);
  });

  it("updates the size at most twice a second", async () => {
    await renderUi(<></>);
    const written = installPicker();
    const stream = streamedExport();
    startExport(job);
    expect(await notifications().findAllByText("Exporting request logs")).not.toHaveLength(0);
    stream.push(1024);
    await waitFor(() => expect(written).toHaveLength(1));
    stream.push(1024);
    await waitFor(() => expect(written).toHaveLength(2));
    expect(await notifications().findAllByText("Exporting request logs, 1.0 KB")).not.toHaveLength(0);
    expect(notifications().queryByText("Exporting request logs, 2.0 KB")).not.toBeInTheDocument();
    stream.end();
    expect(await notifications().findByText(/^2\.0 KB in /)).toBeInTheDocument();
  });

  it("stops when the person cancels it", async () => {
    const { user } = await renderUi(<></>);
    installPicker();
    streamedExport();
    startExport(job);
    expect(await notifications().findAllByText("Exporting request logs")).not.toHaveLength(0);
    await user.click(await notifications().findByRole("button", { name: "Cancel", hidden: true }));
    expect(await notifications().findByText("Export canceled")).toBeInTheDocument();
  });

  it("hands the file to the browser when it can't pick a place to save", async () => {
    await renderUi(<></>);
    on("GET", CHECK, json({ data: [], has_more: false }));
    const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => undefined);
    startExport(job);
    expect(await notifications().findByText("Export started")).toBeInTheDocument();
    expect(notifications().getByText("Your browser shows its progress.")).toBeInTheDocument();
    expect(click).toHaveBeenCalledOnce();
  });

  it("shows the API's message when the export is refused", async () => {
    await renderUi(<></>);
    on("GET", CHECK, apiError(403, "permission_denied", "You can't export these logs."));
    startExport(job);
    expect(await notifications().findAllByText("You can't export these logs.")).not.toHaveLength(0);
  });

  it("says the export stopped when the download breaks unexpectedly", async () => {
    await renderUi(<></>);
    on("GET", CHECK, json({ data: [], has_more: false }));
    vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {
      throw new Error("blocked");
    });
    startExport(job);
    expect(await notifications().findAllByText("The export stopped before the end; try again.")).not.toHaveLength(0);
  });

  it("does nothing when the person closes the save dialog", async () => {
    await renderUi(<></>);
    let close: (() => void) | undefined;
    installPicker(
      () =>
        new Promise((_, reject) => {
          close = () => reject(new DOMException("closed", "AbortError"));
        }),
    );
    startExport(job);
    await waitFor(() => expect(close).toBeDefined());
    expect(showExportProgress()).toBe(true);
    close?.();
    await finished();
    expect(notifications().queryByText("Exporting request logs")).not.toBeInTheDocument();
    expect(requests("GET", EXPORT)).toHaveLength(0);
  });
});
