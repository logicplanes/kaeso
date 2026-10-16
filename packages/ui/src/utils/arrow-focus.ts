import type { KeyboardEvent } from "react";

const FOCUSABLE = 'button:not([disabled]), input:not([disabled]), [tabindex]:not([tabindex="-1"]), a[href]';

export function arrowFocus(e: KeyboardEvent<HTMLElement>) {
  if (e.defaultPrevented || (e.key !== "ArrowLeft" && e.key !== "ArrowRight")) return;
  const target = e.target as HTMLElement;
  if (target.tagName === "INPUT" && (target as HTMLInputElement).value) return;
  const items = [...e.currentTarget.querySelectorAll<HTMLElement>(FOCUSABLE)];
  const i = items.indexOf(target);
  if (i < 0) return;
  e.preventDefault();
  items[(i + (e.key === "ArrowRight" ? 1 : -1) + items.length) % items.length]?.focus();
}
