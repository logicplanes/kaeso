import { afterEach, beforeEach, vi } from "vitest";
import { NOW } from "./fixtures";

export function atFixtureTime(): void {
  beforeEach(() => vi.setSystemTime(new Date(NOW)));
  afterEach(() => vi.useRealTimers());
}
