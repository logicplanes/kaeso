import { expect } from "vitest";

/** A button that can't be pressed yet and says why: it stays focusable, and screen readers hear the reason. */
export function expectDisabledBecause(button: HTMLElement, reason: string): void {
  expect(button).toHaveAttribute("aria-disabled", "true");
  expect(button).toHaveAttribute("aria-description", reason);
}
