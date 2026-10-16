import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { lazyModule, preload, useLazyModule } from "./lazy-module";

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

  it("ignores a failed load in a component", async () => {
    const m = lazyModule(() => Promise.reject(new Error("offline")));
    function Probe() {
      return <span>{useLazyModule(m, true) ?? "waiting"}</span>;
    }
    const { unmount } = render(<Probe />);
    await Promise.resolve();
    expect(screen.getByText("waiting")).toBeInTheDocument();
    unmount();
  });
});
