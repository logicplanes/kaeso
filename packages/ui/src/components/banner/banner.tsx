import { forwardRef, useState, type ReactNode } from "react";
import { cn } from "../../utils/cn";
import { icons } from "../../icons/vocabulary";
import { Icon, type IconData } from "../icon/icon";
import { IconButton } from "../icon-button/icon-button";
import { useUiText } from "../../i18n/context";

export const KAESO_BANNER_VARIANTS = {
  tone: {
    neutral: { classes: "bg-fill-1 text-fg", description: "A neutral notice (default)" },
    info: { classes: "bg-info-tint text-fg", description: "Information" },
    success: { classes: "bg-success-tint text-fg", description: "Something worked" },
    warning: { classes: "bg-warning-tint text-fg", description: "Attention needed soon" },
    danger: { classes: "bg-danger-tint text-fg", description: "Something failed or will fail" },
  },
  layout: {
    inline: { classes: "rounded-well px-16 py-12", description: "Inside content: radius 14 (default)" },
    full: { classes: "rounded-none px-24 py-10", description: "Across the top of the panel for global notices" },
  },
} as const;
export const KAESO_BANNER_DEFAULT_VARIANTS = { tone: "neutral", layout: "inline" } as const;

const ICON: Record<keyof typeof KAESO_BANNER_VARIANTS.tone, IconData> = {
  neutral: icons.info,
  info: icons.info,
  success: icons.success,
  warning: icons.warning,
  danger: icons.error,
};
const ICON_TONE = {
  neutral: "text-fg-secondary",
  info: "text-info-text",
  success: "text-success-text",
  warning: "text-warning-text",
  danger: "text-danger-text",
} as const;

export interface BannerProps {
  tone?: keyof typeof KAESO_BANNER_VARIANTS.tone | undefined;
  layout?: keyof typeof KAESO_BANNER_VARIANTS.layout | undefined;
  title?: ReactNode | undefined;
  children?: ReactNode | undefined;
  icon?: IconData | undefined;
  actions?: ReactNode | undefined;
  onDismiss?: (() => void) | undefined;
  className?: string | undefined;
}

export const Banner = forwardRef<HTMLDivElement, BannerProps>(function Banner(
  { tone = "neutral", layout = "inline", title, children, icon, actions, onDismiss, className },
  ref,
) {
  const [hidden, setHidden] = useState(false);
  const t = useUiText();
  if (hidden) return null;
  const dismissible = onDismiss && (tone === "neutral" || tone === "info");
  return (
    <div
      ref={ref}
      role={tone === "danger" ? "alert" : "status"}
      className={cn("@container w-full", KAESO_BANNER_VARIANTS.tone[tone].classes, KAESO_BANNER_VARIANTS.layout[layout].classes, className)}
    >
      <div className={cn("flex items-start gap-8", actions && "@banner:items-center")}>
        <div className="flex min-w-0 flex-1 flex-col gap-12 @banner:flex-row @banner:items-center @banner:gap-24">
          <div className="flex min-w-0 flex-1 items-start gap-8">
            <Icon icon={icon ?? ICON[tone]} size="sm" className={cn("mt-2", ICON_TONE[tone])} />
            <div className="flex min-w-0 flex-1 flex-col gap-2 text-body">
              {title && <span className="font-semibold text-fg">{title}</span>}
              {children && <div className="text-fg text-pretty">{children}</div>}
            </div>
          </div>
          {actions && <div className="ml-24 flex shrink-0 flex-wrap items-center gap-8 @banner:ml-0">{actions}</div>}
        </div>
        {dismissible && (
          <IconButton
            icon={icons.close}
            aria-label={t("banner.dismiss", "Dismiss")}
            size="xs"
            className="-my-2 -mr-6"
            onClick={() => {
              setHidden(true);
              onDismiss?.();
            }}
          />
        )}
      </div>
    </div>
  );
});
Banner.displayName = "Banner";
