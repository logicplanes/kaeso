import { forwardRef, useLayoutEffect, useRef, useState, type FocusEvent } from "react";
import { cn } from "../../utils/cn.js";
import { icons } from "../../icons/vocabulary.js";
import { Button } from "../button/button.js";
import { IconButton } from "../icon-button/icon-button.js";
import { useUiText } from "../../i18n/context.js";

export const KAESO_PAGINATION_VARIANTS = {
  variant: {
    "load-more": {
      classes: "",
      description:
        "A “Load more” pill while more rows exist; once all are shown, nothing, or “All rows shown” where the button had focus (cursor APIs, default)",
    },
    cursor: { classes: "", description: "Previous and next circles, without a count" },
    pages: { classes: "", description: "Page numbers in seven fixed slots, so Next stays under the pointer, for lists with a known number of pages" },
  },
} as const;
export const KAESO_PAGINATION_DEFAULT_VARIANTS = { variant: "load-more" } as const;

export interface PaginationProps {
  variant?: keyof typeof KAESO_PAGINATION_VARIANTS.variant | undefined;
  hasMore?: boolean | undefined;
  hasPrevious?: boolean | undefined;
  loading?: boolean | undefined;
  onLoadMore?: (() => void) | undefined;
  onNext?: (() => void) | undefined;
  onPrevious?: (() => void) | undefined;
  page?: number | undefined;
  pageCount?: number | undefined;
  onPageChange?: ((page: number) => void) | undefined;
  className?: string | undefined;
}

const SLOTS = 7;

function pageSlots(page: number, pageCount: number): (number | "gap")[] {
  const count = Math.max(1, Math.floor(pageCount));
  const current = Math.min(count, Math.max(1, Math.floor(page)));
  if (count <= SLOTS) return Array.from({ length: count }, (_, i) => i + 1);
  if (current <= 4) return [1, 2, 3, 4, 5, "gap", count];
  if (current >= count - 3) return [1, "gap", count - 4, count - 3, count - 2, count - 1, count];
  return [1, "gap", current - 1, current, current + 1, "gap", count];
}

function soft(blocked: boolean, run: (() => void) | undefined) {
  return {
    ...(blocked ? { "aria-disabled": true } : {}),
    onClick: () => {
      if (!blocked) run?.();
    },
  };
}

export const Pagination = forwardRef<HTMLElement, PaginationProps>(function Pagination(
  { variant = "load-more", hasMore, hasPrevious, loading, onLoadMore, onNext, onPrevious, page = 1, pageCount = 1, onPageChange, className },
  ref,
) {
  const t = useUiText();
  const label = t("pagination.label", "Pagination");
  const previous = t("pagination.previous", "Previous page");
  const next = t("pagination.next", "Next page");

  if (variant === "load-more") {
    return <LoadMore ref={ref} label={label} hasMore={!!hasMore} loading={loading} onLoadMore={onLoadMore} className={className} />;
  }
  if (variant === "cursor") {
    if (!hasMore && !hasPrevious) return null;
    return (
      <nav ref={ref} aria-label={label} className={cn("flex items-center gap-8", className)}>
        <IconButton icon={icons.chevronLeft} aria-label={previous} variant="secondary" {...soft(!hasPrevious, onPrevious)} />
        <IconButton icon={icons.chevronRight} aria-label={next} variant="secondary" {...soft(!hasMore, onNext)} />
      </nav>
    );
  }
  const count = Math.max(1, Math.floor(pageCount));
  const current = Math.min(count, Math.max(1, Math.floor(page)));
  return (
    <nav ref={ref} aria-label={label} className={cn("flex items-center gap-4", className)}>
      <IconButton icon={icons.chevronLeft} aria-label={previous} variant="secondary" {...soft(current <= 1, () => onPageChange?.(current - 1))} />
      <span className="flex items-center gap-4 px-4">
        {pageSlots(current, count).map((p, i) =>
          p === "gap" ? (
            <span key={`gap-${i}`} aria-hidden className="inline-flex h-32 min-w-40 items-center justify-center text-body text-fg-tertiary">
              …
            </span>
          ) : (
            <button
              key={`slot-${i}`}
              type="button"
              aria-current={p === current ? "page" : undefined}
              aria-label={t("pagination.page", "Page {page}", { page: String(p) })}
              onClick={() => {
                if (p !== page) onPageChange?.(p);
              }}
              className={cn(
                "relative inline-flex h-32 min-w-40 cursor-pointer items-center justify-center rounded-pill px-8 text-body font-medium tabular transition-control tap-target focus-ring-inset",
                p === current ? "bg-fill-2 text-fg" : "text-fg-secondary hover:bg-fill-1 hover:text-fg active:bg-fill-2",
              )}
            >
              {p}
            </button>
          ),
        )}
      </span>
      <IconButton icon={icons.chevronRight} aria-label={next} variant="secondary" {...soft(current >= count, () => onPageChange?.(current + 1))} />
    </nav>
  );
});
Pagination.displayName = "Pagination";

const LoadMore = forwardRef<
  HTMLElement,
  { label: string; hasMore: boolean; loading: boolean | undefined; onLoadMore: (() => void) | undefined; className: string | undefined }
>(function PaginationLoadMore({ label, hasMore, loading, onLoadMore, className }, ref) {
  const t = useUiText();
  const loadMore = t("pagination.loadMore", "Load more");
  const [inside, setInside] = useState(false);
  const [ended, setEnded] = useState(false);
  const caption = useRef<HTMLParagraphElement>(null);
  if (hasMore && ended) setEnded(false);
  if (!hasMore && inside && !ended) setEnded(true);
  useLayoutEffect(() => {
    if (ended) caption.current?.focus();
  }, [ended]);
  if (!hasMore && !ended) return null;
  return (
    <nav
      ref={ref}
      aria-label={label}
      className={cn("flex items-center justify-center gap-12 py-12", className)}
      onFocus={() => setInside(true)}
      onBlur={(e: FocusEvent<HTMLElement>) => {
        if (!e.currentTarget.contains(e.relatedTarget)) setInside(false);
      }}
    >
      {hasMore ? (
        <Button variant="secondary" size="sm" loading={loading} onClick={onLoadMore} reserveLabels={[loadMore]}>
          {loadMore}
        </Button>
      ) : (
        <p ref={caption} tabIndex={-1} className="flex h-32 items-center text-small text-fg-secondary outline-none">
          {t("pagination.allShown", "All rows shown")}
        </p>
      )}
    </nav>
  );
});
LoadMore.displayName = "Pagination.LoadMore";
