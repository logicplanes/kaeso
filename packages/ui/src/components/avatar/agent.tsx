import { forwardRef, type CSSProperties, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";

export const AGENT_TILE = {
  20: { classes: "size-20 rounded-avatar", glyph: "size-14", stroke: 2.25 },
  24: { classes: "size-24 rounded-avatar", glyph: "size-16", stroke: 2 },
  28: { classes: "size-28 rounded-org", glyph: "size-18", stroke: 2 },
  32: { classes: "size-32 rounded-org", glyph: "size-20", stroke: 1.75 },
  40: { classes: "size-40 rounded-org", glyph: "size-28", stroke: 1.5 },
} as const;

const AGENT_BADGE = {
  20: { classes: "size-8", px: 8, glyph: "size-6", stroke: 3.5 },
  24: { classes: "size-10", px: 10, glyph: "size-8", stroke: 3 },
  28: { classes: "size-12", px: 12, glyph: "size-10", stroke: 2.75 },
  32: { classes: "size-12", px: 12, glyph: "size-10", stroke: 2.75 },
  40: { classes: "size-16", px: 16, glyph: "size-12", stroke: 2.5 },
} as const;

type AvatarSize = keyof typeof AGENT_TILE;

// Below 14px the ears merge with the head, so the badge drops them and widens the head instead.
const BADGE_BOT = [
  "M6.5 7h11A3.5 3.5 0 0 1 21 10.5v6a3.5 3.5 0 0 1-3.5 3.5h-11A3.5 3.5 0 0 1 3 16.5v-6A3.5 3.5 0 0 1 6.5 7z",
  "M12 7V3.5",
  "M9 13v1.5",
  "M15 13v1.5",
];
const BOT = [
  "M7.5 8h9A3.5 3.5 0 0 1 20 11.5v5a3.5 3.5 0 0 1-3.5 3.5h-9A3.5 3.5 0 0 1 4 16.5v-5A3.5 3.5 0 0 1 7.5 8z",
  "M12 8V4.5",
  "M9.5 13.5v1",
  "M14.5 13.5v1",
  "M2 13v2",
  "M22 13v2",
];

function botGlyph(tile: { glyph: string; stroke: number }, paths: readonly string[] = BOT) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={tile.stroke}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
      focusable="false"
      className={tile.glyph}
    >
      {paths.map((d) => (
        <path key={d} d={d} />
      ))}
    </svg>
  );
}

export function AgentGlyph({ size }: { size: AvatarSize }) {
  return botGlyph(AGENT_TILE[size]);
}

export const AgentBadge = forwardRef<HTMLSpanElement, { size: AvatarSize; label: string; className?: string | undefined; children: ReactNode }>(
  function AgentBadge({ size, label, className, children }, ref) {
    const badge = AGENT_BADGE[size];
    const centre = `${size - badge.px / 2}px`;
    return (
      <span ref={ref} role="img" aria-label={label} className={cn("relative inline-flex shrink-0", className)}>
        <span
          className="flex cutout rounded-pill"
          style={{ "--k-cut-x-ltr": centre, "--k-cut-y": centre, "--k-cut-r": `${badge.px / 2 + 2}px` } as CSSProperties}
        >
          {children}
        </span>
        <span
          aria-hidden
          data-agent-badge=""
          className={cn(
            "absolute right-0 bottom-0 inline-flex items-center justify-center rounded-pill bg-ink text-on-ink data-mark forced-colors:outline forced-colors:-outline-offset-1",
            badge.classes,
          )}
        >
          {botGlyph(badge, BADGE_BOT)}
        </span>
      </span>
    );
  },
);
AgentBadge.displayName = "Avatar";
