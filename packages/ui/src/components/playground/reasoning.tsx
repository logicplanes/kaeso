import { forwardRef, type ReactNode } from "react";
import { Collapsible as BaseCollapsible } from "@base-ui/react/collapsible";
import { cn } from "../../utils/cn.js";
import { icons } from "../../icons/vocabulary.js";
import { formatDuration } from "../../format/index.js";
import { Icon } from "../icon/icon.js";
import { Markdown } from "../markdown/markdown.js";
import { useUiText } from "../../i18n/context.js";

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
  { text = "", live = false, duration, title, hint, open, defaultOpen, onOpenChange, children, className },
  ref,
) {
  const t = useUiText();
  const preview = hint ?? (live ? lastLine(text) : "");
  const heading =
    title ??
    (!live && duration !== undefined
      ? t("responseView.thoughtFor", "Thought for {duration}", { duration: formatDuration(duration) })
      : t("responseView.thinking", "Thinking"));
  return (
    <BaseCollapsible.Root
      ref={ref}
      {...(open !== undefined ? { open } : {})}
      {...(defaultOpen !== undefined ? { defaultOpen } : {})}
      {...(onOpenChange ? { onOpenChange: (o: boolean) => onOpenChange(o) } : {})}
      className={cn("flex min-w-0 flex-col", className)}
    >
      <BaseCollapsible.Trigger className="group/think flex h-32 min-w-0 cursor-pointer items-center gap-6 rounded-pill px-12 text-left text-small text-fg-secondary transition-control hover:bg-fill-1 hover:text-fg focus-ring-inset">
        <span className={cn("shrink-0 font-medium", live && "animate-pulse-quiet reduced:animate-none")}>{heading}</span>
        <Icon
          icon={icons.chevronRight}
          size="xs"
          className="shrink-0 text-fg-tertiary transition-transform duration-180 ease-overlay group-data-panel-open/think:rotate-90"
        />
        <span aria-hidden={hint === undefined} className="min-w-0 flex-1 truncate text-fg-tertiary group-data-panel-open/think:invisible">
          {preview}
        </span>
      </BaseCollapsible.Trigger>
      <BaseCollapsible.Panel className="collapse-motion">
        <div className="flex gap-12 pt-4 pb-8 pl-12">
          <span aria-hidden className="w-1 shrink-0 rounded-pill bg-line" />
          {children ?? <Markdown text={text} size="small" tone="secondary" streaming={live} collapseAfter={0} wrapCode className="min-w-0 flex-1" />}
        </div>
      </BaseCollapsible.Panel>
    </BaseCollapsible.Root>
  );
});
Reasoning.displayName = "Reasoning";
