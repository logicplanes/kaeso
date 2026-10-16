import type { MouseEvent } from "react";

export function focusInputOnPress(e: MouseEvent<HTMLElement>) {
  if (e.target !== e.currentTarget) return;
  const input = e.currentTarget.querySelector("input:not([type=hidden])") as HTMLInputElement | null;
  if (!input || input.disabled) return;
  e.preventDefault();
  input.focus();
}
