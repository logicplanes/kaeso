import { forwardRef } from "react";
import { cn } from "../../utils/cn.js";
import { useUiText } from "../../i18n/context.js";
import { Avatar } from "../avatar/avatar.js";
import { modelName } from "./model-name.js";

export const KAESO_AGENT_AVATAR_VARIANTS = {
  size: {
    20: { classes: "size-20 rounded-avatar", glyph: "size-14", stroke: 2.25, description: "20px" },
    24: { classes: "size-24 rounded-avatar", glyph: "size-16", stroke: 2, description: "24px: rows and lists (default)" },
    28: { classes: "size-28 rounded-org", glyph: "size-18", stroke: 2, description: "28px" },
    32: { classes: "size-32 rounded-org", glyph: "size-20", stroke: 1.75, description: "32px" },
    40: { classes: "size-40 rounded-org", glyph: "size-28", stroke: 1.5, description: "40px: an agent's own page, menus" },
  },
} as const;
export const KAESO_AGENT_AVATAR_DEFAULT_VARIANTS = { size: 24 } as const;

export const KAESO_AGENT_ATTRIBUTION_VARIANTS = {
  size: {
    20: { classes: "size-10 right-0 bottom-0 rounded-cell", glyph: "size-8", stroke: 3, description: "20px person, 10px agent badge" },
    24: { classes: "size-12 right-0 bottom-0 rounded-cell", glyph: "size-10", stroke: 2.75, description: "24px person, 12px agent badge (default)" },
    28: { classes: "size-14 right-0 bottom-0 rounded-cell", glyph: "size-10", stroke: 2.5, description: "28px person, 14px agent badge" },
    32: { classes: "size-16 right-0 bottom-0 rounded-cell", glyph: "size-12", stroke: 2.25, description: "32px person, 16px agent badge" },
    40: { classes: "size-20 right-0 bottom-0 rounded-avatar", glyph: "size-14", stroke: 2, description: "40px person, 20px agent badge" },
  },
} as const;
export const KAESO_AGENT_ATTRIBUTION_DEFAULT_VARIANTS = { size: 24 } as const;

type AvatarSize = keyof typeof KAESO_AGENT_AVATAR_VARIANTS.size;

// A bot's face on a 24 grid: head 4–20, ears at 2 and 22, antenna to 4.5, eyes as round-capped dots. Badges
// drop the ears and widen the head, which is all that still reads at 8 to 14 pixels.
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

export interface AgentAvatarProps {
  name: string;
  size?: AvatarSize | undefined;
  decorative?: boolean | undefined;
  className?: string | undefined;
}

export const AgentAvatar = forwardRef<HTMLSpanElement, AgentAvatarProps>(function AgentAvatar({ name, size = 24, decorative, className }, ref) {
  const tile = KAESO_AGENT_AVATAR_VARIANTS.size[size];
  return (
    <span
      ref={ref}
      {...(decorative ? { "aria-hidden": true } : { role: "img", "aria-label": name })}
      className={cn("inline-flex shrink-0 items-center justify-center bg-ink text-on-ink select-none", tile.classes, className)}
    >
      {botGlyph(tile)}
    </span>
  );
});
AgentAvatar.displayName = "AgentAvatar";

export interface AgentAttributionProps {
  name: string;
  src?: string | undefined;
  agent: { name: string; model: string };
  size?: AvatarSize | undefined;
  className?: string | undefined;
}

export const AgentAttribution = forwardRef<HTMLSpanElement, AgentAttributionProps>(function AgentAttribution({ name, src, agent, size = 24, className }, ref) {
  const t = useUiText();
  const badge = KAESO_AGENT_ATTRIBUTION_VARIANTS.size[size];
  return (
    <span
      ref={ref}
      role="img"
      aria-label={t("agentAttribution.label", "{name} via {agent} ({model})", { name, agent: agent.name, model: modelName(agent.model, t) })}
      className={cn("relative inline-flex shrink-0", className)}
    >
      <Avatar name={name} {...(src ? { src } : {})} size={size} decorative />
      <span
        aria-hidden
        data-agent-badge=""
        className={cn("absolute inline-flex items-center justify-center bg-ink text-on-ink ring-2 ring-panel", badge.classes)}
      >
        {botGlyph(badge, BADGE_BOT)}
      </span>
    </span>
  );
});
AgentAttribution.displayName = "AgentAttribution";
