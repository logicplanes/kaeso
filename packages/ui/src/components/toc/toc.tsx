import { forwardRef, useEffect, useState } from "react";
import { cn } from "../../utils/cn.js";
import { useUiText } from "../../i18n/context.js";

export const KAESO_TABLE_OF_CONTENTS_VARIANTS = {
  state: {
    default: { classes: "text-fg-tertiary hover:text-fg", description: "A heading, faded until it is read or pointed at" },
    active: {
      classes: "text-fg",
      description: "The heading in view, at full strength, its row's hairline drawn in ink (the only line left in forced colours)",
    },
  },
} as const;
export const KAESO_TABLE_OF_CONTENTS_DEFAULT_VARIANTS = { state: "default" } as const;

export interface TocItem {
  id: string;
  label: string;
  depth?: 0 | 1 | undefined;
}

export interface TableOfContentsProps {
  items: TocItem[];
  title?: string | undefined;
  active?: string | undefined;
  className?: string | undefined;
}

const READING_LINE = 0.3;

function scrollParent(el: HTMLElement): Element {
  for (let parent = el.parentElement; parent; parent = parent.parentElement) {
    if (/(auto|scroll)/.test(getComputedStyle(parent).overflowY) && parent.scrollHeight > parent.clientHeight) return parent;
  }
  return document.scrollingElement ?? document.documentElement;
}

export const TableOfContents = forwardRef<HTMLElement, TableOfContentsProps>(function TableOfContents({ items, title: titleProp, active, className }, ref) {
  const t = useUiText();
  const title = titleProp ?? t("tableOfContents.title", "On this page");
  const [seen, setSeen] = useState(items[0]?.id);
  useEffect(() => {
    if (active !== undefined) return;
    const headings = items.map((i) => document.getElementById(i.id)).filter((el): el is HTMLElement => el !== null);
    const first = headings[0];
    if (!first) return;
    const scroller = scrollParent(first);
    let frame = 0;
    const pick = () => {
      frame = 0;
      const page = scroller === document.scrollingElement || scroller === document.documentElement;
      const view = page ? { top: 0, height: window.innerHeight } : { top: scroller.getBoundingClientRect().top, height: scroller.clientHeight };
      const line = view.top + view.height * READING_LINE;
      let id = first.id;
      for (const h of headings) if (h.getBoundingClientRect().top <= line) id = h.id;
      const end = scroller.scrollTop > 0 && scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 2;
      setSeen(end ? (headings.at(-1)?.id ?? id) : id);
    };
    const schedule = () => {
      frame ||= requestAnimationFrame(pick);
    };
    pick();
    document.addEventListener("scroll", schedule, { capture: true, passive: true });
    window.addEventListener("resize", schedule);
    return () => {
      cancelAnimationFrame(frame);
      document.removeEventListener("scroll", schedule, { capture: true });
      window.removeEventListener("resize", schedule);
    };
  }, [items, active]);
  const current = active ?? seen;
  return (
    <nav ref={ref} aria-label={title} className={cn("flex flex-col", className)}>
      <span className="pb-8 text-small font-medium text-fg-secondary">{title}</span>
      <ul className="flex flex-col hairline-t-strong">
        {items.map((i) => {
          const on = i.id === current;
          return (
            <li key={i.id} className={cn("relative flex hairline-b-strong", i.depth && "ms-12")}>
              <span aria-hidden className={cn("absolute inset-x-0 bottom-0 h-1 bg-ink transition-control forced-line", !on && "opacity-0")} />
              <a
                href={`#${i.id}`}
                aria-current={on ? "location" : undefined}
                className={cn(
                  "flex min-h-32 w-full items-center rounded-item py-8 pe-8 text-small transition-control focus-ring coarse:min-h-44",
                  KAESO_TABLE_OF_CONTENTS_VARIANTS.state[on ? "active" : "default"].classes,
                )}
              >
                {i.label}
              </a>
            </li>
          );
        })}
      </ul>
    </nav>
  );
});
TableOfContents.displayName = "TableOfContents";
