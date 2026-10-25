import { forwardRef } from "react";
import { cn } from "../../utils/cn.js";
import { useUiText } from "../../i18n/context.js";
import { Avatar } from "../avatar/avatar.js";
import { agentHarness, HARNESS_GLYPHS } from "./harness-glyphs.js";
import { harnessName } from "./harness-name.js";

export const KAESO_AGENT_AVATAR_VARIANTS = {
  size: {
    20: { classes: "size-20 rounded-avatar", glyph: "size-16", stroke: 2, description: "20px" },
    24: { classes: "size-24 rounded-avatar", glyph: "size-20", stroke: 2, description: "24px: rows and lists (default)" },
    28: { classes: "size-28 rounded-org", glyph: "size-22", stroke: 2, description: "28px" },
    32: { classes: "size-32 rounded-org", glyph: "size-24", stroke: 2, description: "32px" },
    40: { classes: "size-40 rounded-org", glyph: "size-32", stroke: 1.75, description: "40px: an agent's own page, menus" },
  },
} as const;
export const KAESO_AGENT_AVATAR_DEFAULT_VARIANTS = { size: 24 } as const;

export const KAESO_AGENT_ATTRIBUTION_VARIANTS = {
  size: {
    20: { classes: "size-10 -right-3 -bottom-3 rounded-cell", glyph: "size-10", stroke: 2.75, description: "20px person, 10px agent badge" },
    24: { classes: "size-12 -right-4 -bottom-4 rounded-cell", glyph: "size-12", stroke: 2.5, description: "24px person, 12px agent badge (default)" },
    28: { classes: "size-14 -right-4 -bottom-4 rounded-cell", glyph: "size-12", stroke: 2.5, description: "28px person, 14px agent badge" },
    32: { classes: "size-16 -right-5 -bottom-5 rounded-cell", glyph: "size-14", stroke: 2.25, description: "32px person, 16px agent badge" },
    40: { classes: "size-20 -right-6 -bottom-6 rounded-avatar", glyph: "size-16", stroke: 2, description: "40px person, 20px agent badge" },
  },
} as const;
export const KAESO_AGENT_ATTRIBUTION_DEFAULT_VARIANTS = { size: 24 } as const;

type AvatarSize = keyof typeof KAESO_AGENT_AVATAR_VARIANTS.size;

function harnessGlyph(harness: string, tile: { glyph: string; stroke: number }) {
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
      data-harness={agentHarness(harness)}
      className={tile.glyph}
    >
      {HARNESS_GLYPHS[agentHarness(harness)].map((d) => (
        <path key={d} d={d} />
      ))}
    </svg>
  );
}

export interface AgentAvatarProps {
  harness: string;
  name: string;
  size?: AvatarSize | undefined;
  decorative?: boolean | undefined;
  className?: string | undefined;
}

export const AgentAvatar = forwardRef<HTMLSpanElement, AgentAvatarProps>(function AgentAvatar({ harness, name, size = 24, decorative, className }, ref) {
  const tile = KAESO_AGENT_AVATAR_VARIANTS.size[size];
  return (
    <span
      ref={ref}
      {...(decorative ? { "aria-hidden": true } : { role: "img", "aria-label": name })}
      className={cn("inline-flex shrink-0 items-center justify-center bg-fill-2 text-fg select-none", tile.classes, className)}
    >
      {harnessGlyph(harness, tile)}
    </span>
  );
});
AgentAvatar.displayName = "AgentAvatar";

export interface AgentAttributionProps {
  name: string;
  src?: string | undefined;
  agent: { name: string; harness: string };
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
      aria-label={t("agentAttribution.label", "{name} via {agent} ({harness})", { name, agent: agent.name, harness: harnessName(agent.harness, t) })}
      className={cn("relative inline-flex shrink-0", className)}
    >
      <Avatar name={name} {...(src ? { src } : {})} size={size} decorative />
      <span
        aria-hidden
        data-agent-badge=""
        className={cn("absolute inline-flex items-center justify-center bg-fill-2 text-fg ring-2 ring-panel", badge.classes)}
      >
        {harnessGlyph(agent.harness, badge)}
      </span>
    </span>
  );
});
AgentAttribution.displayName = "AgentAttribution";
