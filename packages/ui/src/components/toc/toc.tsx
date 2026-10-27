import { forwardRef, useEffect, useState } from "react";
import { cn } from "../../utils/cn.js";
import { useUiText } from "../../i18n/context.js";

export const KAESO_TABLE_OF_CONTENTS_VARIANTS = {
  state: {
    default: { classes: "text-fg-secondary hover:text-fg", description: "A heading" },
    active: { classes: "text-fg", description: "The heading in view, with a 2px ink bar on the rail (a solid line in forced colours)" },
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
      <span className="mb-8 text-caption font-medium text-fg-secondary">{title}</span>
      <ul className="relative flex flex-col before:absolute before:inset-y-0 before:start-0 before:w-1 before:bg-line-subtle before:forced-line">
        {items.map((i) => (
          <li key={i.id}>
            <a
              href={`#${i.id}`}
              aria-current={i.id === current ? "location" : undefined}
              className={cn(
                "relative ms-4 flex min-h-32 items-center rounded-pill py-4 pe-8 text-small transition-control focus-ring-inset coarse:min-h-44",
                i.depth ? "ps-20" : "ps-8",
                KAESO_TABLE_OF_CONTENTS_VARIANTS.state[i.id === current ? "active" : "default"].classes,
              )}
            >
              {i.id === current && <span aria-hidden className="absolute inset-y-6 -start-4 w-2 rounded-pill bg-ink forced-line" />}
              {i.label}
            </a>
          </li>
        ))}
      </ul>
    </nav>
  );
});
TableOfContents.displayName = "TableOfContents";
