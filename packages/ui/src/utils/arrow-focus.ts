import type { KeyboardEvent } from "react";

const FOCUSABLE = 'button:not([disabled]), input:not([disabled]), [tabindex]:not([tabindex="-1"]), a[href]';

export function arrowFocus(e: KeyboardEvent<HTMLElement>, { byItem = false }: { byItem?: boolean | undefined } = {}) {
  if (e.defaultPrevented || (e.key !== "ArrowLeft" && e.key !== "ArrowRight")) return;
  const target = e.target as HTMLElement;
  if (target.tagName === "INPUT" && (target as HTMLInputElement).value) return;
  const groups = byItem
    ? [...e.currentTarget.children]
        .map((child) => (child.matches(FOCUSABLE) ? [child as HTMLElement] : [...child.querySelectorAll<HTMLElement>(FOCUSABLE)]))
        .filter((g) => g.length)
    : [...e.currentTarget.querySelectorAll<HTMLElement>(FOCUSABLE)].map((el) => [el]);
  const i = groups.findIndex((g) => g.includes(target));
  if (i < 0) return;
  e.preventDefault();
  groups[(i + (e.key === "ArrowRight" ? 1 : -1) + groups.length) % groups.length]?.[0]?.focus();
}
