import { forwardRef } from "react";
import { cn } from "../../utils/cn";
import { icons } from "../../icons/vocabulary";
import { Button } from "../button/button";
import { IconButton } from "../icon-button/icon-button";
import { useUiText } from "../../i18n/context";

export const KAESO_PAGINATION_VARIANTS = {
  variant: {
    "load-more": { classes: "", description: "A “Load more” pill while more rows exist, nothing once all are shown (cursor APIs, default)" },
    cursor: { classes: "", description: "Previous and next circles, without a count" },
    pages: { classes: "", description: "Page numbers, for lists with a known number of pages" },
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

export const Pagination = forwardRef<HTMLElement, PaginationProps>(function Pagination(
  { variant = "load-more", hasMore, hasPrevious, loading, onLoadMore, onNext, onPrevious, page = 1, pageCount = 1, onPageChange, className },
  ref,
) {
  const t = useUiText();
  const label = t("pagination.label", "Pagination");
  const previous = t("pagination.previous", "Previous page");
  const next = t("pagination.next", "Next page");
  const loadMore = t("pagination.loadMore", "Load more");

  if (variant === "load-more") {
    if (!hasMore) return null;
    return (
      <nav ref={ref} aria-label={label} className={cn("flex items-center justify-center gap-12 py-12", className)}>
        <Button variant="ghost" size="sm" loading={loading} onClick={onLoadMore} reserveLabels={[loadMore]}>
          {loadMore}
        </Button>
      </nav>
    );
  }
  if (variant === "cursor") {
    return (
      <nav ref={ref} aria-label={label} className={cn("flex items-center gap-8", className)}>
        <IconButton icon={icons.chevronLeft} aria-label={previous} variant="secondary" disabled={!hasPrevious} onClick={onPrevious} />
        <IconButton icon={icons.chevronRight} aria-label={next} variant="secondary" disabled={!hasMore} onClick={onNext} />
      </nav>
    );
  }
  const pages = Array.from({ length: pageCount }, (_, i) => i + 1).filter((p) => p === 1 || p === pageCount || Math.abs(p - page) <= 1);
  return (
    <nav ref={ref} aria-label={label} className={cn("flex items-center gap-4", className)}>
      <IconButton icon={icons.chevronLeft} aria-label={previous} disabled={page <= 1} onClick={() => onPageChange?.(page - 1)} />
      {pages.map((p, i) => (
        <span key={p} className="flex items-center gap-4">
          {i > 0 && p - (pages[i - 1] ?? p) > 1 && <span className="w-24 text-center text-fg-tertiary">…</span>}
          <button
            type="button"
            aria-current={p === page ? "page" : undefined}
            onClick={() => onPageChange?.(p)}
            className={cn(
              "inline-flex h-32 min-w-32 cursor-pointer items-center justify-center rounded-pill px-8 text-body font-medium tabular transition-control focus-ring",
              p === page ? "bg-fill-2 text-fg" : "text-fg-secondary hover:bg-fill-1 hover:text-fg",
            )}
          >
            {p}
          </button>
        </span>
      ))}
      <IconButton icon={icons.chevronRight} aria-label={next} disabled={page >= pageCount} onClick={() => onPageChange?.(page + 1)} />
    </nav>
  );
});
Pagination.displayName = "Pagination";
