import { describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { lazyModule, lazyModulesSettled, preload, useLazyModule } from "./lazy-module.js";

describe("lazyModule", () => {
  it("loads once and keeps the module", async () => {
    const load = vi.fn(() => Promise.resolve({ name: "shiki" }));
    const m = lazyModule(load);
    expect(m.current()).toBeUndefined();
    await Promise.all([m.load(), m.load()]);
    expect(load).toHaveBeenCalledTimes(1);
    expect(m.current()).toEqual({ name: "shiki" });
  });

  it("tries again after a failed load, and preload swallows the failure", async () => {
    const load = vi.fn().mockRejectedValueOnce(new Error("offline")).mockResolvedValue("ok");
    const m = lazyModule(load);
    preload(m);
    await expect(m.load()).rejects.toThrow("offline");
    await expect(m.load()).resolves.toBe("ok");
  });

  const base = () => "https://kaeso.example.com/assets/page-header.B6XY1N.js";
  const chunk = "https://kaeso.example.com/assets/nav-sheet.C1a2b3.js";
  const bundled = <T,>(load: () => Promise<T>) =>
    Object.assign(load, { toString: () => "() => o(() => import(`./nav-sheet.C1a2b3.js`), __vite__mapDeps([0, 1]))" });

  it("fetches a module whose download failed in Chromium from a fresh address, and asks for no other while the fresh one fails to download", async () => {
    const load = vi.fn(() => Promise.reject(new TypeError(`Failed to fetch dynamically imported module: ${chunk}`)));
    const reimport = vi
      .fn<(url: string) => Promise<string>>()
      .mockRejectedValueOnce(new TypeError(`Failed to fetch dynamically imported module: ${chunk}?retry=1`))
      .mockResolvedValue("sheet");
    const m = lazyModule(load, { reimport });
    await expect(m.load()).rejects.toThrow("Failed to fetch");
    await expect(m.load()).rejects.toThrow("Failed to fetch");
    await expect(m.load()).resolves.toBe("sheet");
    expect(load).toHaveBeenCalledTimes(1);
    expect(reimport.mock.calls).toEqual([[`${chunk}?retry=1`], [`${chunk}?retry=2`]]);
    expect(m.current()).toBe("sheet");
  });

  it("fetches a module whose download failed in WebKit from a fresh address read from its loader, since WebKit names none", async () => {
    const load = vi.fn(() => Promise.reject(new TypeError("Importing a module script failed.")));
    const reimport = vi.fn<(url: string) => Promise<string>>().mockResolvedValue("sheet");
    const m = lazyModule(bundled(load), { base, reimport });
    await expect(m.load()).rejects.toThrow("Importing a module script failed.");
    await expect(m.load()).resolves.toBe("sheet");
    expect(load).toHaveBeenCalledTimes(1);
    expect(reimport.mock.calls).toEqual([[`${chunk}?retry=1`]]);
  });

  it("falls back to its loader when the fresh address can't be imported", async () => {
    const load = vi.fn<() => Promise<string>>().mockRejectedValueOnce(new TypeError("Importing a module script failed.")).mockResolvedValue("sheet");
    const reimport = vi.fn<(url: string) => Promise<string>>().mockRejectedValue(new TypeError("Only URLs with a scheme in: file and data are supported"));
    const m = lazyModule(bundled(load), { base, reimport });
    await expect(m.load()).rejects.toThrow("Importing a module script failed.");
    await expect(m.load()).resolves.toBe("sheet");
    expect(reimport.mock.calls).toEqual([[`${chunk}?retry=1`]]);
    expect(load).toHaveBeenCalledTimes(2);
  });

  it.each([
    ["WebKit", new TypeError("Importing a module script failed.")],
    [
      "Firefox, naming a chunk the module imports",
      new TypeError("error loading dynamically imported module: https://kaeso.example.com/assets/dialog-root.D4e5f6.js"),
    ],
    ["the module itself", new Error("Kaeso is not defined")],
  ])("loads a module the same way again after a failure from %s when neither the error nor the loader names its address", async (_, failure) => {
    const load = vi.fn<() => Promise<string>>().mockRejectedValueOnce(failure).mockResolvedValue("sheet");
    const reimport = vi.fn<(url: string) => Promise<string>>();
    const m = lazyModule(load, { base, reimport });
    await expect(m.load()).rejects.toThrow(failure.message);
    await expect(m.load()).resolves.toBe("sheet");
    expect(load).toHaveBeenCalledTimes(2);
    expect(reimport).not.toHaveBeenCalled();
  });

  it("renders once the module arrives, only when it is wanted", async () => {
    const m = lazyModule(() => Promise.resolve("loaded"));
    function Probe({ wanted }: { wanted: boolean }) {
      return <span>{useLazyModule(m, wanted) ?? "waiting"}</span>;
    }
    const { rerender } = render(<Probe wanted={false} />);
    expect(screen.getByText("waiting")).toBeInTheDocument();
    rerender(<Probe wanted />);
    await waitFor(() => expect(screen.getByText("loaded")).toBeInTheDocument());
  });

  it("settles once every module asked for has arrived and the component that asked for it has drawn it", async () => {
    let arrive!: (value: string) => void;
    const m = lazyModule(() => new Promise<string>((resolve) => (arrive = resolve)));
    function Probe() {
      return <span>{useLazyModule(m, true) ?? "waiting"}</span>;
    }
    render(<Probe />);
    let settled = false;
    const done = act(async () => {
      await lazyModulesSettled();
      settled = true;
    });
    await Promise.resolve();
    expect(settled).toBe(false);
    arrive("loaded");
    await done;
    expect(screen.getByText("loaded")).toBeInTheDocument();
  });

  it("ignores a failed load in a component and never tries again on its own", async () => {
    const load = vi.fn(() => Promise.reject(new Error("offline")));
    const m = lazyModule(load);
    function Probe() {
      return <span>{useLazyModule(m, true) ?? "waiting"}</span>;
    }
    const { rerender, unmount } = render(<Probe />);
    await act(() => lazyModulesSettled());
    rerender(<Probe />);
    await act(() => lazyModulesSettled());
    expect(screen.getByText("waiting")).toBeInTheDocument();
    expect(load).toHaveBeenCalledTimes(1);
    unmount();
  });

  it("tells the component that wanted a module when its load failed, and loads it again when it is wanted anew", async () => {
    const load = vi.fn<() => Promise<string>>().mockRejectedValueOnce(new Error("offline")).mockResolvedValue("loaded");
    const m = lazyModule(load);
    function Probe() {
      const [wanted, setWanted] = useState(true);
      const [failures, setFailures] = useState(0);
      const mod = useLazyModule(m, wanted, () => {
        setFailures((n) => n + 1);
        setWanted(false);
      });
      return (
        <>
          <span>{mod ?? "waiting"}</span>
          <span>{`${failures} failed`}</span>
          <button onClick={() => setWanted(true)}>Open</button>
        </>
      );
    }
    render(<Probe />);
    expect(await screen.findByText("1 failed")).toBeInTheDocument();
    expect(load).toHaveBeenCalledTimes(1);
    await userEvent.click(screen.getByRole("button", { name: "Open" }));
    expect(await screen.findByText("loaded")).toBeInTheDocument();
    expect(screen.getByText("1 failed")).toBeInTheDocument();
    expect(load).toHaveBeenCalledTimes(2);
  });

  it("forgets a failed load once the component no longer wants it", async () => {
    let fail!: (error: Error) => void;
    const m = lazyModule(() => new Promise<string>((_, reject) => (fail = reject)));
    const onFail = vi.fn();
    function Probe({ wanted }: { wanted: boolean }) {
      return <span>{useLazyModule(m, wanted, onFail) ?? "waiting"}</span>;
    }
    const { rerender } = render(<Probe wanted />);
    rerender(<Probe wanted={false} />);
    await act(async () => {
      fail(new Error("offline"));
      await lazyModulesSettled();
    });
    expect(onFail).not.toHaveBeenCalled();
    expect(screen.getByText("waiting")).toBeInTheDocument();
  });
});
