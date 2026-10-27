import { forwardRef, useState, useSyncExternalStore, type CSSProperties, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { useUiText } from "../../i18n/context.js";
import { Tooltip } from "../tooltip/tooltip.js";

const noSubscription = () => () => undefined;
const notHydrating = () => false;
const isHydrating = () => true;

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
  const [failedSrc, setFailedSrc] = useState<string | undefined>(undefined);
  const hydrating = useSyncExternalStore(noSubscription, notHydrating, isHydrating);
  return (
    <span
      ref={ref}
      {...(decorative ? { "aria-hidden": true } : { role: "img", "aria-label": name })}
      className={cn(
        "relative inline-flex shrink-0 items-center justify-center overflow-hidden mark-fill font-semibold text-fg select-none forced-colors:outline forced-colors:-outline-offset-1",
        KAESO_AVATAR_VARIANTS.size[size].classes,
        KAESO_AVATAR_VARIANTS.kind[kind].classes,
        size <= 24 && kind === "org" && "rounded-avatar",
        className,
      )}
    >
      {src && src !== failedSrc ? (
        <>
          <img
            ref={(img) => {
              // A server-rendered picture that failed before hydration fired its error event before React listened for it.
              if (hydrating && img?.complete && img.naturalWidth === 0) setFailedSrc(src);
            }}
            src={src}
            alt=""
            className="size-full object-cover"
            onError={() => setFailedSrc(src)}
          />
          <span aria-hidden className="pointer-events-none absolute inset-0 shadow-ring" />
        </>
      ) : (
        initials(name, size === 20 ? 1 : 2)
      )}
    </span>
  );
});
Avatar.displayName = "Avatar";

export interface AvatarGroupProps {
  people: { name: string; src?: string | undefined }[];
  size?: keyof typeof KAESO_AVATAR_VARIANTS.size | undefined;
  max?: number | undefined;
  className?: string | undefined;
  children?: ReactNode | undefined;
}

const GROUP_OVERLAP = { 20: 4, 24: 4, 28: 4, 32: 6, 40: 8 } as const;
const OVERLAP_CLASS = { 4: "-ms-4", 6: "-ms-6", 8: "-ms-8" } as const;
const GAP = 2;

function cutFor(size: number, overlap: number): CSSProperties {
  return {
    "--k-cut-x-ltr": `${1.5 * size - overlap}px`,
    "--k-cut-x-rtl": `${overlap - 0.5 * size}px`,
    "--k-cut-r": `${size / 2 + GAP}px`,
  } as CSSProperties;
}

export const AvatarGroup = forwardRef<HTMLDivElement, AvatarGroupProps>(function AvatarGroup({ people, size = 24, max = 4, className, children }, ref) {
  const t = useUiText();
  const shown = people.slice(0, max);
  const rest = people.length - shown.length;
  const overlap = GROUP_OVERLAP[size];
  const items = shown.length + (rest > 0 ? 1 : 0);
  const slot = (i: number, child: ReactNode, key: string) => (
    <span
      key={key}
      className={cn("flex shrink-0 rounded-pill", i > 0 && OVERLAP_CLASS[overlap], i < items - 1 && "cutout")}
      style={i < items - 1 ? cutFor(size, overlap) : undefined}
    >
      {child}
    </span>
  );
  return (
    <div
      ref={ref}
      role="group"
      aria-label={people.map((p) => p.name).join(t("avatarGroup.listSeparator", ", "))}
      className={cn("flex items-center", className)}
    >
      {shown.map((p, i) => slot(i, <Avatar name={p.name} {...(p.src ? { src: p.src } : {})} size={size} decorative />, p.name + i))}
      {rest > 0 &&
        slot(
          shown.length,
          <Tooltip
            content={people
              .slice(max)
              .map((p) => p.name)
              .join(t("avatarGroup.listSeparator", ", "))}
            shape="multiline"
          >
            <span
              className={cn(
                "inline-flex shrink-0 items-center justify-center rounded-pill mark-fill font-medium text-fg-secondary tabular forced-colors:outline forced-colors:-outline-offset-1",
                KAESO_AVATAR_VARIANTS.size[size].classes,
              )}
            >
              +{rest}
            </span>
          </Tooltip>,
          "rest",
        )}
      {children && <span className="ms-8 inline-flex items-center">{children}</span>}
    </div>
  );
});
AvatarGroup.displayName = "AvatarGroup";
