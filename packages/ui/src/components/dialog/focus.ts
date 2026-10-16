import { KAESO_BUTTON_VARIANTS } from "../button/button.js";

const DIALOG_FIELD =
  'input:not([type=hidden]):not([tabindex="-1"]):not([aria-hidden="true"]), textarea, select, [role=combobox]:not([tabindex="-1"]), [role=radio][tabindex="0"], [role=checkbox]:not([tabindex="-1"]), [role=switch]:not([tabindex="-1"]), [role=listbox]:not([tabindex="-1"]), [role=grid] [data-index][tabindex="0"]';
const FOCUSABLE = 'a[href], button, input:not([type=hidden]), select, textarea, [tabindex]:not([tabindex="-1"])';

const usable = (el: HTMLElement) => !el.closest("[hidden], [inert]") && !el.matches(":disabled, [aria-disabled=true]");
const firstUsable = (scope: ParentNode, selector: string) => [...scope.querySelectorAll<HTMLElement>(selector)].find(usable) ?? null;

export function initialDialogControl(frame: HTMLElement, scope?: HTMLElement | undefined): HTMLElement | null {
  const content = scope ?? frame.querySelector<HTMLElement>("[data-dialog-body]");
  const footer = frame.querySelector<HTMLElement>("[data-dialog-footer]");
  const actions = footer ? [...footer.querySelectorAll<HTMLElement>("button, a[href]")].filter(usable) : [];
  const { primary, danger, "danger-secondary": dangerSecondary } = KAESO_BUTTON_VARIANTS.variant;
  const safe = actions.filter((a) => !a.classList.contains(danger.classes) && !a.classList.contains(dangerSecondary.classes));
  return (
    (content && (firstUsable(content, "[data-autofocus]") ?? firstUsable(content, DIALOG_FIELD))) ??
    safe.find((a) => a.classList.contains(primary.classes)) ??
    safe[0] ??
    (content && firstUsable(content, FOCUSABLE)) ??
    firstUsable(frame, FOCUSABLE)
  );
}

export function focusDialogSection(section: HTMLElement | null | undefined, previous?: HTMLElement | null | undefined) {
  const frame = section?.closest<HTMLElement>("[data-dialog-frame]");
  if (!section || !frame || frame.closest("[data-closed]")) return;
  const back = previous?.isConnected && frame.contains(previous) && usable(previous) ? previous : null;
  (back ?? initialDialogControl(frame, section))?.focus({ preventScroll: true });
}
