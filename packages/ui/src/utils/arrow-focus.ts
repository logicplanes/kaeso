import type { KeyboardEvent } from "react";

const CANDIDATES = 'button, input, select, textarea, a[href], [tabindex], [contenteditable]:not([contenteditable="false"])';

export function focusables(container: HTMLElement): HTMLElement[] {
  return [...container.querySelectorAll<HTMLElement>(CANDIDATES)].filter(
    (el) =>
      !el.matches(":disabled") &&
      (el.getAttribute("tabindex") !== "-1" || el.dataset.roving !== undefined) &&
      !el.closest("[inert], [hidden], [aria-hidden='true']") &&
      (typeof el.checkVisibility !== "function" || el.checkVisibility()),
  );
}

export function arrowFocus(e: KeyboardEvent<HTMLElement>, { byItem = false }: { byItem?: boolean | undefined } = {}) {
  if (e.defaultPrevented || (e.key !== "ArrowLeft" && e.key !== "ArrowRight")) return;
  const target = e.target as HTMLElement;
  if (target.tagName === "INPUT" && (target as HTMLInputElement).value) return;
  const all = focusables(e.currentTarget);
  const groups = byItem ? [...e.currentTarget.children].map((child) => all.filter((el) => child.contains(el))).filter((g) => g.length) : all.map((el) => [el]);
  const i = groups.findIndex((g) => g.includes(target));
  if (i < 0) return;
  e.preventDefault();
  const step = e.key === "ArrowRight" ? 1 : -1;
  for (let n = 1; n < groups.length; n++) {
    const next = groups[(((i + step * n) % groups.length) + groups.length) % groups.length]?.[0];
    next?.focus();
    if (next && document.activeElement === next) return;
  }
}
