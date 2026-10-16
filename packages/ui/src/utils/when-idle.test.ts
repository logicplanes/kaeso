import { describe, expect, it, vi } from "vitest";
import { whenIdle } from "./when-idle.js";

describe("whenIdle", () => {
  it("runs once the browser is idle, or soon where it can't tell, and not after it is cancelled", () => {
    const requests: { run: () => void; timeout: number | undefined }[] = [];
    vi.stubGlobal("requestIdleCallback", (run: () => void, options?: { timeout?: number }) => requests.push({ run, timeout: options?.timeout }));
    vi.stubGlobal("cancelIdleCallback", vi.fn());
    const idle = vi.fn();
    const cancel = whenIdle(idle);
    expect(requests).toEqual([{ run: idle, timeout: 2000 }]);
    cancel();
    expect(cancelIdleCallback).toHaveBeenCalledWith(1);
    vi.unstubAllGlobals();

    vi.useFakeTimers();
    const soon = vi.fn();
    whenIdle(soon);
    vi.advanceTimersByTime(299);
    expect(soon).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(soon).toHaveBeenCalledTimes(1);
    const never = vi.fn();
    whenIdle(never)();
    vi.runAllTimers();
    expect(never).not.toHaveBeenCalled();
    vi.useRealTimers();
  });
});
