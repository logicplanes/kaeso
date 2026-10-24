import { forwardRef, useEffect, useState } from "react";
import { cn } from "../../utils/cn.js";
import { useUiText } from "../../i18n/context.js";

export const KAESO_TABLE_OF_CONTENTS_VARIANTS = {
  state: {
    default: { classes: "text-fg-secondary hover:text-fg", description: "A heading" },
    active: { classes: "text-fg", description: "The heading in view, with a 2px ink bar" },
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
      const line = window.innerHeight * READING_LINE;
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
      <ul className="flex flex-col hairline-l">
        {items.map((i) => (
          <li key={i.id}>
            <a
              href={`#${i.id}`}
              aria-current={i.id === current ? "location" : undefined}
              className={cn(
                "relative flex min-h-28 items-center py-4 text-small transition-control focus-ring-inset rounded-code",
                i.depth ? "pl-24" : "pl-12",
                KAESO_TABLE_OF_CONTENTS_VARIANTS.state[i.id === current ? "active" : "default"].classes,
              )}
            >
              {i.id === current && <span aria-hidden className="absolute inset-y-4 left-0 w-2 rounded-pill bg-ink" />}
              {i.label}
            </a>
          </li>
        ))}
      </ul>
    </nav>
  );
});
TableOfContents.displayName = "TableOfContents";
