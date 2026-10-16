import { forwardRef, useState, type ReactNode } from "react";
import { cn } from "../../utils/cn";
import { useUiText } from "../../i18n/context";

export const KAESO_AVATAR_VARIANTS = {
  size: {
    20: { classes: "size-20 text-micro", description: "20px" },
    24: { classes: "size-24 text-micro", description: "24px: rows and the org switcher (default)" },
    28: { classes: "size-28 text-caption", description: "28px" },
    32: { classes: "size-32 text-caption", description: "32px" },
    40: { classes: "size-40 text-body", description: "40px: menus, profiles" },
  },
  kind: {
    person: { classes: "rounded-pill", description: "People are round" },
    org: { classes: "rounded-org", description: "Organizations and projects: radius 8" },
  },
} as const;
export const KAESO_AVATAR_DEFAULT_VARIANTS = { size: 24, kind: "person" } as const;

export function initials(name: string, max: 1 | 2 = 2): string {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) return "?";
  const first = parts[0]?.[0] ?? "";
  return (max === 1 || parts.length < 2 ? first : first + (parts[parts.length - 1]?.[0] ?? "")).toUpperCase();
}

export interface AvatarProps {
  name: string;
  src?: string | undefined;
  size?: keyof typeof KAESO_AVATAR_VARIANTS.size | undefined;
  kind?: keyof typeof KAESO_AVATAR_VARIANTS.kind | undefined;
  decorative?: boolean | undefined;
  className?: string | undefined;
}

export const Avatar = forwardRef<HTMLSpanElement, AvatarProps>(function Avatar({ name, src, size = 24, kind = "person", decorative, className }, ref) {
  const [failed, setFailed] = useState(false);
  return (
    <span
      ref={ref}
      {...(decorative ? { "aria-hidden": true } : { role: "img", "aria-label": name })}
      className={cn(
        "relative inline-flex shrink-0 items-center justify-center overflow-hidden bg-fill-2 font-semibold text-fg select-none",
        KAESO_AVATAR_VARIANTS.size[size].classes,
        KAESO_AVATAR_VARIANTS.kind[kind].classes,
        size <= 24 && kind === "org" && "rounded-avatar",
        className,
      )}
    >
      {src && !failed ? <img src={src} alt="" className="size-full object-cover" onError={() => setFailed(true)} /> : initials(name, size === 20 ? 1 : 2)}
    </span>
  );
});
Avatar.displayName = "Avatar";

export interface AvatarGroupProps {
  people: { name: string; src?: string }[];
  size?: keyof typeof KAESO_AVATAR_VARIANTS.size | undefined;
  max?: number | undefined;
  className?: string | undefined;
  children?: ReactNode | undefined;
}

const GROUP_OVERLAP = { 20: "-ms-4", 24: "-ms-2", 28: "-ms-2", 32: "-ms-4", 40: "-ms-6" } as const;

export const AvatarGroup = forwardRef<HTMLDivElement, AvatarGroupProps>(function AvatarGroup({ people, size = 24, max = 4, className }, ref) {
  const t = useUiText();
  const shown = people.slice(0, max);
  const rest = people.length - shown.length;
  const overlap = GROUP_OVERLAP[size];
  return (
    <div
      ref={ref}
      role="group"
      aria-label={people.map((p) => p.name).join(t("avatarGroup.listSeparator", ", "))}
      className={cn("flex items-center", className)}
    >
      {shown.map((p, i) => (
        <Avatar
          key={p.name + i}
          name={p.name}
          {...(p.src ? { src: p.src } : {})}
          size={size}
          decorative
          className={cn("ring-2 ring-panel", i > 0 && overlap)}
        />
      ))}
      {rest > 0 && (
        <span
          className={cn(
            "inline-flex shrink-0 items-center justify-center rounded-pill bg-fill-1 font-medium text-fg-secondary ring-2 ring-panel tabular",
            KAESO_AVATAR_VARIANTS.size[size].classes,
            overlap,
          )}
        >
          +{rest}
        </span>
      )}
    </div>
  );
});
AvatarGroup.displayName = "AvatarGroup";
