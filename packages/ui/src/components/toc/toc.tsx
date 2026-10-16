import { forwardRef, useEffect, useState } from "react";
import { cn } from "../../utils/cn";
import { useUiText } from "../../i18n/context";

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

export const TableOfContents = forwardRef<HTMLElement, TableOfContentsProps>(function TableOfContents({ items, title: titleProp, active, className }, ref) {
  const t = useUiText();
  const title = titleProp ?? t("tableOfContents.title", "On this page");
  const [seen, setSeen] = useState(items[0]?.id);
  useEffect(() => {
    if (active !== undefined || typeof IntersectionObserver === "undefined") return;
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) if (e.isIntersecting) setSeen(e.target.id);
      },
      { rootMargin: "0px 0px -70% 0px" },
    );
    items.forEach((i) => {
      const el = document.getElementById(i.id);
      if (el) io.observe(el);
    });
    return () => io.disconnect();
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
