import { useCallback, useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { Button } from "../button/button";
import { Dialog, KAESO_DIALOG_VARIANTS } from "./dialog";
import { useUiText } from "../../i18n/context";

export interface DialogStep {
  id: string;
  title: ReactNode;
  description?: ReactNode | undefined;
  content: ReactNode;
  blockedReason?: string | undefined;
}

export interface StepDialogProps {
  open?: boolean | undefined;
  onOpenChange?: ((open: boolean) => void) | undefined;
  title: ReactNode;
  steps: DialogStep[];
  submitLabel: string;
  onSubmit: () => void;
  submitting?: boolean | undefined;
  tone?: "default" | "danger" | undefined;
  size?: keyof typeof KAESO_DIALOG_VARIANTS.size | undefined;
  inline?: boolean | undefined;
  defaultStep?: number | undefined;
}

const STEP_FIELD =
  '[data-autofocus], input:not([type=hidden]):not([disabled]):not([tabindex="-1"]):not([aria-hidden="true"]), textarea:not([disabled]), select:not([disabled]), [role=combobox]:not([disabled]):not([tabindex="-1"]), [role=radio][tabindex="0"], [role=checkbox]:not([disabled]):not([tabindex="-1"]), [role=switch]:not([disabled]):not([tabindex="-1"]), [role=listbox]:not([tabindex="-1"])';

function focusStep(section: HTMLElement | null | undefined) {
  if (!section) return;
  const field = section.querySelector<HTMLElement>("[data-autofocus]") ?? section.querySelector<HTMLElement>(STEP_FIELD);
  (field ?? section.querySelector<HTMLElement>("h3"))?.focus({ preventScroll: true });
}

const GRID = 4;
const KEEP = 16;

const px = (v: string) => parseFloat(v) || 0;

function reducedMotion() {
  return typeof window !== "undefined" && !!window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
}

function stepHeight(content: number, current: number | null) {
  const next = Math.ceil((content - 0.5) / GRID) * GRID;
  if (current === null || next > current || current - next > KEEP) return next;
  return current;
}

function panelOf(frame: HTMLElement) {
  const popup = frame.closest<HTMLElement>("[role=dialog]");
  const viewport = popup?.parentElement;
  return popup && viewport ? { popup, viewport } : null;
}

function snapHeight(frame: HTMLElement, height: number) {
  frame.style.transition = "none";
  frame.style.height = `${height}px`;
  void frame.offsetHeight;
  frame.style.transition = "";
}

function fit(frame: HTMLElement, height: number, inline: boolean | undefined, instant: boolean) {
  delete frame.dataset.full;
  if (instant) {
    frame.style.height = `${height}px`;
    return;
  }
  const panel = inline ? null : panelOf(frame);
  const body = frame.parentElement;
  const shown = px(frame.style.height);
  const fcs = getComputedStyle(frame);
  const moves = fcs.transitionProperty.includes("height") && px(fcs.transitionDuration) > 0 && !reducedMotion();
  let limit = Infinity;
  let top: number | null = null;
  let anchor: number | null = null;
  if (panel && body) {
    const { popup, viewport } = panel;
    const vs = getComputedStyle(viewport);
    const room = viewport.clientHeight - px(vs.paddingTop) - px(vs.paddingBottom);
    const max = Math.min(px(getComputedStyle(popup).maxHeight) || room, room);
    const bs = getComputedStyle(body);
    const rest = popup.offsetHeight - body.offsetHeight + px(bs.paddingTop) + px(bs.paddingBottom) + px(fcs.marginTop) + px(fcs.marginBottom);
    limit = Math.max(0, max - rest);
    const inset = Math.max(0, (room - max) / 2);
    const held = popup.dataset.stepTop;
    anchor = held !== undefined ? Number(held) : Math.round(popup.offsetTop - px(vs.paddingTop));
    top = Math.round(Math.max(inset, Math.min(anchor, room - inset - Math.min(max, rest + height))));
  }
  const visible = moves ? Math.min(height, limit) : height;
  if (panel && top !== null && anchor !== null) {
    const { popup } = panel;
    if (popup.dataset.stepTop === undefined) {
      popup.style.setProperty("--k-step-top", `${anchor}px`);
      popup.dataset.stepTop = String(anchor);
      popup.setAttribute("data-step-anchored", "");
      void popup.offsetTop;
      popup.setAttribute("data-step-glide", "");
    }
    popup.style.setProperty("--k-step-top", `${top}px`);
  }
  if (moves && shown > limit) snapHeight(frame, limit);
  if (visible < height) {
    if (Math.min(shown, limit) === visible) return snapHeight(frame, height);
    frame.dataset.full = String(height);
  }
  frame.style.height = `${visible}px`;
}

function settle(frame: HTMLElement) {
  const full = frame.dataset.full;
  if (full === undefined) return;
  delete frame.dataset.full;
  snapHeight(frame, Number(full));
}

function release(frame: HTMLElement) {
  const popup = panelOf(frame)?.popup;
  if (!popup) return;
  popup.removeAttribute("data-step-glide");
  popup.removeAttribute("data-step-anchored");
  popup.style.removeProperty("--k-step-top");
  delete popup.dataset.stepTop;
}

export function StepDialog({
  open,
  onOpenChange,
  title,
  steps,
  submitLabel,
  onSubmit,
  submitting,
  tone = "default",
  size = "sm",
  inline,
  defaultStep = 0,
}: StepDialogProps) {
  const [index, setIndex] = useState(defaultStep);
  const t = useUiText();
  const [announced, setAnnounced] = useState(false);
  const [enter, setEnter] = useState<"forward" | "back" | null>(null);
  const [opened, setOpened] = useState({ open, defaultStep });
  if (opened.open !== open || opened.defaultStep !== defaultStep) {
    setOpened({ open, defaultStep });
    if (open || opened.defaultStep !== defaultStep) {
      setIndex(defaultStep);
      setAnnounced(false);
      setEnter(null);
    }
  }
  const sections = useRef<(HTMLElement | null)[]>([]);
  const [frame, setFrame] = useState<HTMLDivElement | null>(null);
  const content = useRef<HTMLDivElement | null>(null);
  const height = useRef<number | null>(null);
  const moved = useRef(false);
  const go = (to: number) => {
    moved.current = true;
    setAnnounced(true);
    setEnter(reducedMotion() ? null : to > index ? "forward" : "back");
    setIndex(to);
  };
  useLayoutEffect(() => {
    const inner = content.current;
    if (!frame || !inner) return;
    const first = height.current === null;
    const next = stepHeight(inner.offsetHeight, first ? null : height.current);
    height.current = next;
    fit(frame, next, inline, first);
    if (!first && frame.parentElement) frame.parentElement.scrollTop = 0;
  }, [frame, index, inline]);
  useEffect(() => {
    const inner = content.current;
    if (!frame || !inner || typeof ResizeObserver === "undefined") return;
    let pending = 0;
    const grow = () => {
      pending = 0;
      if (height.current === null) return;
      const next = stepHeight(inner.offsetHeight, height.current);
      if (next <= height.current) return;
      height.current = next;
      fit(frame, next, inline, false);
    };
    const ro = new ResizeObserver(() => {
      if (!pending) pending = requestAnimationFrame(grow);
    });
    ro.observe(inner);
    const resize = () => release(frame);
    const end = (e: TransitionEvent) => {
      if (e.target === frame && e.propertyName === "height") settle(frame);
    };
    if (!inline) window.addEventListener("resize", resize);
    frame.addEventListener("transitionend", end);
    return () => {
      ro.disconnect();
      cancelAnimationFrame(pending);
      window.removeEventListener("resize", resize);
      frame.removeEventListener("transitionend", end);
    };
  }, [frame, inline]);
  const setFrameRef = useCallback((el: HTMLDivElement | null) => {
    if (!el) height.current = null;
    setFrame(el);
  }, []);
  useEffect(() => {
    if (!moved.current) return;
    moved.current = false;
    focusStep(sections.current[index]);
  }, [index]);
  const step = steps[Math.min(index, steps.length - 1)];
  const last = index >= steps.length - 1;
  const blocked = step?.blockedReason;
  const next = () => {
    if (blocked || submitting) return;
    if (last) onSubmit();
    else go(index + 1);
  };
  const progress = { current: index + 1, total: steps.length };
  const continueLabel = t("stepDialog.continue", "Continue");
  const [heardBefore, heardAfter = ""] = t("stepDialog.stepAnnouncement", "Step {current} of {total}: {title}", progress).split("{title}");
  return (
    <Dialog
      {...(open !== undefined ? { open } : {})}
      {...(onOpenChange ? { onOpenChange } : {})}
      {...(inline ? { inline } : {})}
      size={size}
      title={title}
      description={steps.length > 1 ? t("stepDialog.stepOf", "Step {current} of {total}", progress) : undefined}
      onSubmit={next}
      footer={
        <>
          {index > 0 ? (
            <Button onClick={() => go(index - 1)}>{t("stepDialog.back", "Back")}</Button>
          ) : (
            <Button onClick={() => onOpenChange?.(false)}>{t("stepDialog.cancel", "Cancel")}</Button>
          )}
          <Button
            variant={last && tone === "danger" ? "danger" : "primary"}
            loading={last && submitting}
            disabled={!!blocked}
            {...(blocked ? { disabledReason: blocked } : {})}
            onClick={next}
            reserveLabels={[submitLabel, continueLabel]}
          >
            {last ? submitLabel : continueLabel}
          </Button>
        </>
      }
    >
      <p className="sr-only" aria-live="polite">
        {announced && step && (
          <>
            {heardBefore}
            {step.title}
            {heardAfter}
          </>
        )}
      </p>
      <div ref={setFrameRef} data-step-frame="" className="-m-8 overflow-clip step-height-motion">
        <div ref={content} className="p-8">
          {steps.map((s, i) => (
            <section
              key={s.id}
              ref={(el) => {
                sections.current[i] = el;
              }}
              aria-labelledby={`step-${s.id}`}
              {...(i !== index ? { hidden: true } : enter ? { "data-enter": enter } : {})}
              className="flex min-w-0 flex-col gap-16 step-motion"
              onKeyDown={(e: KeyboardEvent) => {
                const target = e.target as HTMLElement;
                if (e.key === "Enter" && !e.metaKey && !e.ctrlKey && target.tagName === "INPUT" && target.getAttribute("role") !== "combobox") {
                  e.preventDefault();
                  next();
                }
              }}
            >
              <div className="flex flex-col gap-4">
                <h3 id={`step-${s.id}`} tabIndex={-1} className="text-body-lg font-semibold text-fg outline-none">
                  {s.title}
                </h3>
                {s.description && <p className="text-small text-fg-secondary">{s.description}</p>}
              </div>
              {s.content}
            </section>
          ))}
        </div>
      </div>
    </Dialog>
  );
}
StepDialog.displayName = "StepDialog";
