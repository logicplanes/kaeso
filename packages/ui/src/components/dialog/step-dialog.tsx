import { useEffect, useId, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { Button } from "../button/button.js";
import { Dialog, KAESO_DIALOG_VARIANTS } from "./dialog.js";
import { focusDialogSection } from "./focus.js";
import { useUiText } from "../../i18n/context.js";

export interface DialogStep {
  id: string;
  title: ReactNode;
  description?: ReactNode | undefined;
  content: ReactNode;
  blockedReason?: string | undefined;
}

export interface StepDialogResult {
  title: ReactNode;
  description?: ReactNode | undefined;
  content: ReactNode;
  footer: ReactNode;
  size?: keyof typeof KAESO_DIALOG_VARIANTS.size | undefined;
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
  result?: StepDialogResult | undefined;
}

function reducedMotion() {
  return typeof window !== "undefined" && !!window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
}

const bodyOf = (el: HTMLElement | null | undefined) => el?.closest<HTMLElement>("[data-dialog-body]") ?? null;

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
  result,
}: StepDialogProps) {
  const [index, setIndex] = useState(defaultStep);
  const uid = useId();
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
  const resultSection = useRef<HTMLElement | null>(null);
  const moved = useRef(false);
  const shown = useRef(index);
  const go = (to: number) => {
    moved.current = true;
    setAnnounced(true);
    setEnter(reducedMotion() ? null : to > index ? "forward" : "back");
    setIndex(to);
  };
  useLayoutEffect(() => {
    if (shown.current === index) return;
    shown.current = index;
    const body = bodyOf(sections.current[index]);
    if (body) body.scrollTop = 0;
  }, [index]);
  const finished = !!result;
  useLayoutEffect(() => {
    if (!finished) return;
    const body = bodyOf(resultSection.current);
    if (body) body.scrollTop = 0;
    focusDialogSection(resultSection.current);
  }, [finished]);
  useEffect(() => {
    if (!moved.current) return;
    moved.current = false;
    focusDialogSection(sections.current[index]);
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
  const stepList = (
    <div className="-m-8 overflow-clip p-8">
      {steps.map((s, i) => (
        <section
          key={s.id}
          ref={(el) => {
            sections.current[i] = el;
          }}
          aria-labelledby={`${uid}-step-${s.id}`}
          {...(i !== index ? { hidden: true } : enter ? { "data-enter": enter } : {})}
          className="flex min-w-0 flex-col gap-16 step-motion"
        >
          <div className="flex flex-col gap-4">
            <h3 id={`${uid}-step-${s.id}`} tabIndex={-1} className="text-body-lg font-semibold text-fg outline-none">
              {s.title}
            </h3>
            {s.description && <p className="text-small text-fg-secondary">{s.description}</p>}
          </div>
          {s.content}
        </section>
      ))}
    </div>
  );
  const frame = {
    ...(open !== undefined ? { open } : {}),
    ...(onOpenChange ? { onOpenChange } : {}),
    ...(inline ? { inline } : {}),
  };
  if (result)
    return (
      <Dialog {...frame} anchor="top" size={result.size ?? size} title={result.title} description={result.description} footer={result.footer}>
        <p className="sr-only" aria-live="polite">
          {result.title}
        </p>
        <div className="-m-8 overflow-clip p-8">
          <section ref={resultSection} {...(reducedMotion() ? {} : { "data-enter": "forward" })} className="flex min-w-0 flex-col step-motion">
            {result.content}
          </section>
        </div>
      </Dialog>
    );
  return (
    <Dialog
      {...frame}
      anchor="top"
      busy={!!submitting}
      size={size}
      title={title}
      description={steps.length > 1 ? t("stepDialog.stepOf", "Step {current} of {total}", progress) : undefined}
      onSubmit={next}
      footer={
        <>
          {index > 0 ? (
            <Button onClick={() => go(index - 1)} disabled={!!submitting}>
              {t("stepDialog.back", "Back")}
            </Button>
          ) : (
            <Button onClick={() => onOpenChange?.(false)} disabled={!!submitting}>
              {t("stepDialog.cancel", "Cancel")}
            </Button>
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
      {stepList}
    </Dialog>
  );
}
StepDialog.displayName = "StepDialog";
