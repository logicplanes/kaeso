import { forwardRef, type ReactNode } from "react";
import { formatDuration } from "../../format/index.js";
import { Markdown } from "../markdown/markdown.js";
import { useUiText } from "../../i18n/context.js";
import { ReasoningFrame } from "./reasoning-frame.js";

export const KAESO_REASONING_VARIANTS = {
  state: {
    done: { classes: "", description: "Finished: “Thought for 4.2 s”, closed until opened (default)" },
    live: { classes: "", description: "Still reasoning: “Thinking” pulses quietly and the latest line shows beside it" },
  },
} as const;
export const KAESO_REASONING_DEFAULT_VARIANTS = { state: "done" } as const;

export interface ReasoningProps {
  text?: string | undefined;
  live?: boolean | undefined;
  duration?: number | undefined;
  title?: ReactNode | undefined;
  hint?: ReactNode | undefined;
  open?: boolean | undefined;
  defaultOpen?: boolean | undefined;
  onOpenChange?: ((open: boolean) => void) | undefined;
  children?: ReactNode | undefined;
  className?: string | undefined;
  "data-force"?: string | undefined;
}

function lastLine(text: string) {
  const lines = text.trim().split("\n");
  for (let i = lines.length - 1; i >= 0; i--) {
    const l = (lines[i] ?? "").replace(/[*_`#>]+/g, "").trim();
    if (l) return l;
  }
  return "";
}

export const Reasoning = forwardRef<HTMLDivElement, ReasoningProps>(function Reasoning(
  { text = "", live = false, duration, title, hint, open, defaultOpen, onOpenChange, children, className, "data-force": force },
  ref,
) {
  const t = useUiText();
  const heading =
    title ??
    (!live && duration !== undefined
      ? t("responseView.thoughtFor", "Thought for {duration}", { duration: formatDuration(duration) })
      : t("responseView.thinking", "Thinking"));
  return (
    <ReasoningFrame
      ref={ref}
      heading={heading}
      preview={hint ?? (live ? lastLine(text) : "")}
      previewHidden={hint === undefined}
      live={live}
      open={open}
      defaultOpen={defaultOpen}
      onOpenChange={onOpenChange}
      className={className}
      data-force={force}
    >
      {children ?? <Markdown text={text} size="small" tone="secondary" streaming={live} collapseAfter={0} wrapCode className="min-w-0" />}
    </ReasoningFrame>
  );
});
Reasoning.displayName = "Reasoning";
