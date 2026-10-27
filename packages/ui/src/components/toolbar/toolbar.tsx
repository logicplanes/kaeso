import { forwardRef, useEffect, useMemo, useRef, type HTMLAttributes, type KeyboardEvent } from "react";
import { cn } from "../../utils/cn.js";
import { useInset } from "../../utils/inset.js";
import { arrowFocus, focusables } from "../../utils/arrow-focus.js";
import { mergeRefs } from "../../utils/merge-refs.js";

export const KAESO_TOOLBAR_VARIANTS = {
  size: {
    md: {
      classes: "min-h-48 gap-8 px-24 py-6",
      description: "48px, 24px side padding, under the page header (default); one tab stop, arrows move between controls",
    },
    inset: { classes: "min-h-48 gap-8 px-0 py-6", description: "48px without side padding, inside padded content" },
  },
} as const;
export const KAESO_TOOLBAR_DEFAULT_VARIANTS = { size: "md" } as const;

export interface ToolbarProps extends HTMLAttributes<HTMLDivElement> {
  size?: keyof typeof KAESO_TOOLBAR_VARIANTS.size | undefined;
  "aria-label": string;
}

const Root = forwardRef<HTMLDivElement, ToolbarProps>(function Toolbar({ size: sizeProp, className, onKeyDown, onFocus, ...props }, ref) {
  const inset = useInset();
  const size = sizeProp ?? (inset ? "inset" : "md");
  const own = useRef<HTMLDivElement | null>(null);
  const refs = useMemo(() => mergeRefs(own, ref), [ref]);
  const current = useRef<HTMLElement | null>(null);
  useEffect(() => {
    const bar = own.current;
    if (!bar) return;
    const rove = () => {
      const items = focusables(bar);
      const stop = current.current && items.includes(current.current) ? current.current : items[0];
      for (const el of items) {
        el.dataset.roving = "";
        el.tabIndex = el === stop ? 0 : -1;
      }
    };
    rove();
    const watch = new MutationObserver(rove);
    watch.observe(bar, { childList: true, subtree: true, attributes: true, attributeFilter: ["disabled", "hidden"] });
    return () => watch.disconnect();
  }, []);
  return (
    <div
      ref={refs}
      role="toolbar"
      aria-orientation="horizontal"
      className={cn("flex shrink-0 flex-wrap items-center", KAESO_TOOLBAR_VARIANTS.size[size].classes, className)}
      onFocus={(e) => {
        onFocus?.(e);
        const bar = own.current;
        const target = e.target as HTMLElement;
        if (!bar || !focusables(bar).includes(target)) return;
        current.current = target;
        for (const el of focusables(bar)) el.tabIndex = el === target ? 0 : -1;
      }}
      onKeyDown={(e: KeyboardEvent<HTMLDivElement>) => {
        onKeyDown?.(e);
        arrowFocus(e);
      }}
      {...props}
    />
  );
});
Root.displayName = "Toolbar";

const Spacer = forwardRef<HTMLSpanElement>(function ToolbarSpacer(_props, ref) {
  return <span ref={ref} aria-hidden className="flex-1" />;
});
Spacer.displayName = "Toolbar.Spacer";

export const Toolbar = Object.assign(Root, { Spacer });
