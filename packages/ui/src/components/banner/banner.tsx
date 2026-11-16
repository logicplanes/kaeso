import { forwardRef, useState, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { icons } from "../../icons/vocabulary.js";
import { Icon, type IconData } from "../icon/icon.js";
import { Button } from "../button/button.js";
import { useUiText } from "../../i18n/context.js";
import { devWarn } from "../../utils/dev.js";

export const KAESO_BANNER_VARIANTS = {
  tone: {
    neutral: { classes: "bg-fill-1 text-fg", description: "A neutral notice, without an icon (default)" },
    info: { classes: "bg-fill-1 text-fg", description: "Information: a blue mark before the words" },
    success: { classes: "bg-fill-1 text-fg", description: "Something worked: a green mark before the words" },
    warning: { classes: "bg-fill-1 text-fg", description: "Attention needed soon: an amber mark before the words" },
    danger: { classes: "bg-fill-1 text-fg", description: "Something failed or will fail: a red mark before the words" },
  },
  layout: {
    inline: {
      classes: "flex min-h-56 flex-col justify-center rounded-well px-12 py-12",
      description: "Inside content: a cut plate, radius 6, its icon 12px in, at least 56px tall with or without actions (default)",
    },
    full: {
      classes: "rounded-none px-24 py-10 hairline-b-strong",
      description: "Across the top of the panel for global notices: its icon on the 24px gutter, a hairline at its foot",
    },
  },
} as const;
export const KAESO_BANNER_DEFAULT_VARIANTS = { tone: "neutral", layout: "inline" } as const;

const ICON: Record<keyof typeof KAESO_BANNER_VARIANTS.tone, IconData | undefined> = {
  neutral: undefined,
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
  if (onDismiss && !dismissible) devWarn(`Banner: a ${tone} banner can't be dismissed; it stays until its cause is fixed, so onDismiss is ignored.`);
  const mark = icon ?? ICON[tone];
  return (
    <div
      ref={ref}
      role={tone === "danger" ? "alert" : "status"}
      className={cn(
        "@container w-full forced-colors:outline forced-colors:-outline-offset-1",
        KAESO_BANNER_VARIANTS.tone[tone].classes,
        KAESO_BANNER_VARIANTS.layout[layout].classes,
        className,
      )}
    >
      <div className={cn("flex items-start gap-8", actions && "@banner:items-center")}>
        <div className="flex min-w-0 flex-1 flex-col gap-12 @banner:flex-row @banner:items-center @banner:gap-24">
          <div className="flex min-w-0 flex-1 items-start gap-8">
            {mark && <Icon icon={mark} size="sm" className={cn("mt-2", ICON_TONE[tone])} />}
            <div className="flex min-w-0 flex-1 flex-col gap-2 text-body">
              {title && <span className="font-semibold text-fg">{title}</span>}
              {children && <div className={cn("text-pretty", title ? "text-fg-secondary" : "text-fg")}>{children}</div>}
            </div>
          </div>
          {actions && <div className={cn("flex shrink-0 flex-wrap items-center gap-8 @banner:ml-0", mark && "ml-24")}>{actions}</div>}
        </div>
        {dismissible && (
          <Button
            variant="ghost"
            icon={icons.close}
            aria-label={t("banner.dismiss", "Dismiss")}
            size="xs"
            className="-my-2 -mr-6 hover:bg-fill-2 active:bg-fill-3"
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
