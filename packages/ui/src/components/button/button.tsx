import { forwardRef, type ButtonHTMLAttributes, type MouseEvent, type ReactElement, type ReactNode, type Ref } from "react";
import { cn } from "../../utils/cn";
import { useLinkComponent, isExternalHref } from "../../utils/link-provider";
import { warnMissingName, warnStylingClassName } from "../../utils/dev";
import { Icon, type IconData, type IconSize } from "../icon/icon";
import { Spinner, useDelayedLoading } from "../loader/loader";
import { KbdSequence } from "../kbd/kbd";
import { Tooltip } from "../tooltip/tooltip";
import { SwapText } from "../swap-text/swap-text";

export const KAESO_BUTTON_VARIANTS = {
  variant: {
    primary: { classes: "material-primary", description: "Ink with a faint top light. Exactly one per view." },
    secondary: { classes: "material-secondary", description: "Raised, ring and a soft shadow. The default." },
    ghost: { classes: "material-ghost", description: "Transparent until hovered: toolbars, rows, quiet actions" },
    quiet: {
      classes: "material-ghost text-fg-secondary hover:text-fg",
      description: "A ghost in secondary text that darkens on hover: actions inside a field (clear, reveal, copy)",
    },
    soft: { classes: "material-soft", description: "A fill without a ring: the active pill tab" },
    danger: { classes: "material-danger", description: "The destructive confirm, with the primary's material" },
    "danger-secondary": { classes: "material-danger-secondary", description: "Raised with danger text; tints on hover. Destructive triggers." },
    link: {
      classes: "bg-transparent text-fg underline decoration-line-strong decoration-1 underline-offset-3 hover:decoration-current disabled:text-fg-disabled",
      description: "Text with an underline, for actions inside sentences",
    },
  },
  size: {
    xs: { classes: "h-24 px-10 gap-4 text-caption font-medium", description: "24px: chips inside rows and tables only" },
    sm: { classes: "h-32 px-12 gap-6 text-body font-medium", description: "32px: toolbars, headers, settings rows" },
    md: { classes: "h-36 px-16 gap-6 text-body font-medium", description: "36px: the default" },
    lg: { classes: "h-40 px-18 gap-8 text-body font-medium", description: "40px: prominent actions" },
    xl: { classes: "h-48 px-22 gap-8 text-body-lg font-medium", description: "48px: sign-in and setup forms, empty-state actions" },
  },
  shape: {
    pill: { classes: "rounded-pill", description: "Every button is a pill" },
    circle: { classes: "rounded-pill p-0 justify-center", description: "Icon-only: a circle of the same height" },
  },
} as const;
export const KAESO_BUTTON_DEFAULT_VARIANTS = { variant: "secondary", size: "md", shape: "pill" } as const;

export type ButtonVariant = keyof typeof KAESO_BUTTON_VARIANTS.variant;
export type ButtonSize = keyof typeof KAESO_BUTTON_VARIANTS.size;
export type ButtonShape = keyof typeof KAESO_BUTTON_VARIANTS.shape;

const LEADING_ICON_PADDING: Record<ButtonSize, string> = { xs: "pl-8", sm: "pl-10", md: "pl-14", lg: "pl-16", xl: "pl-20" };
const TRAILING_ICON_PADDING: Record<ButtonSize, string> = { xs: "pr-8", sm: "pr-10", md: "pr-14", lg: "pr-16", xl: "pr-20" };
const CIRCLE: Record<ButtonSize, string> = { xs: "size-24", sm: "size-32", md: "size-36", lg: "size-40", xl: "size-48" };
export const BUTTON_ICON_SIZE: Record<ButtonSize, IconSize> = { xs: "xs", sm: "sm", md: "sm", lg: "md", xl: "lg" };

export function buttonClasses({
  variant = KAESO_BUTTON_DEFAULT_VARIANTS.variant,
  size = KAESO_BUTTON_DEFAULT_VARIANTS.size,
  shape = KAESO_BUTTON_DEFAULT_VARIANTS.shape,
  leading = false,
  trailing = false,
  fullWidth = false,
}: { variant?: ButtonVariant; size?: ButtonSize; shape?: ButtonShape; leading?: boolean; trailing?: boolean; fullWidth?: boolean } = {}) {
  return cn(
    "relative inline-flex shrink-0 select-none items-center justify-center whitespace-nowrap no-underline",
    "cursor-pointer transition-control focus-ring tap-target disabled:cursor-not-allowed",
    KAESO_BUTTON_VARIANTS.size[size].classes,
    KAESO_BUTTON_VARIANTS.shape[shape].classes,
    shape === "circle" && CIRCLE[size],
    shape === "pill" && leading && variant !== "link" && LEADING_ICON_PADDING[size],
    shape === "pill" && trailing && variant !== "link" && TRAILING_ICON_PADDING[size],
    variant === "link" && "-mx-4 px-4",
    KAESO_BUTTON_VARIANTS.variant[variant].classes,
    fullWidth && "w-full",
  );
}

interface CommonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children"> {
  variant?: ButtonVariant | undefined;
  size?: ButtonSize | undefined;
  loading?: boolean | undefined;
  disabledReason?: string | undefined;
  href?: string | undefined;
  target?: string | undefined;
  rel?: string | undefined;
  fullWidth?: boolean | undefined;
  "data-force"?: string | undefined;
}

export interface PillButtonProps extends CommonProps {
  shape?: "pill" | undefined;
  children: ReactNode;
  leadingIcon?: IconData | undefined;
  trailingIcon?: IconData | undefined;
  shortcut?: string[] | string[][] | undefined;
  count?: number | undefined;
  reserveLabels?: ReactNode[] | undefined;
}

export interface CircleButtonProps extends CommonProps {
  shape: "circle";
  icon: IconData;
  "aria-label": string;
  children?: never | undefined;
}

export type ButtonProps = PillButtonProps | CircleButtonProps;

export const Button = forwardRef<HTMLButtonElement | HTMLAnchorElement, ButtonProps>(function Button(props, ref) {
  const {
    variant = KAESO_BUTTON_DEFAULT_VARIANTS.variant,
    size = KAESO_BUTTON_DEFAULT_VARIANTS.size,
    loading = false,
    disabled = false,
    disabledReason,
    href,
    target,
    rel,
    fullWidth,
    className,
    type,
    onClick,
    ...rest
  } = props;
  const LinkComp = useLinkComponent();
  warnStylingClassName("Button", className);

  const spinning = useDelayedLoading(loading);
  const busy = loading || spinning;
  const isCircle = props.shape === "circle";
  const iconSize = BUTTON_ICON_SIZE[size];
  const spinnerSize = iconSize === "xs" ? "xs" : iconSize === "sm" ? "sm" : "md";
  const leadingIcon = !isCircle ? (props as PillButtonProps).leadingIcon : undefined;
  const trailingIcon = !isCircle ? (props as PillButtonProps).trailingIcon : undefined;
  const shortcut = !isCircle ? (props as PillButtonProps).shortcut : undefined;
  const count = !isCircle ? (props as PillButtonProps).count : undefined;
  const reserveLabels = !isCircle ? (props as PillButtonProps).reserveLabels : undefined;
  if (isCircle) warnMissingName("Button", !!(props as CircleButtonProps)["aria-label"]);

  const domProps = { ...rest } as Record<string, unknown>;
  for (const k of ["shape", "icon", "leadingIcon", "trailingIcon", "shortcut", "count", "children", "reserveLabels"]) delete domProps[k];

  const softDisabled = !!disabledReason && disabled;
  const inert = busy || softDisabled;
  const classes = cn(
    buttonClasses({
      variant,
      size,
      shape: isCircle ? "circle" : "pill",
      leading: !!leadingIcon,
      trailing: !!trailingIcon || !!shortcut || count !== undefined,
      fullWidth: !!fullWidth,
    }),
    busy && "cursor-progress",
    className,
  );

  const content = isCircle ? (
    spinning ? (
      <Spinner size={spinnerSize} />
    ) : (
      <Icon icon={(props as CircleButtonProps).icon} size={iconSize} />
    )
  ) : (
    <>
      {spinning && leadingIcon ? <Spinner size={spinnerSize} /> : leadingIcon ? <Icon icon={leadingIcon} size={iconSize} /> : null}
      <span className={cn("inline-flex min-w-0 items-center", spinning && !leadingIcon && "opacity-0")}>
        {reserveLabels ? <SwapText reserve={reserveLabels}>{(props as PillButtonProps).children}</SwapText> : (props as PillButtonProps).children}
      </span>
      {count !== undefined && (
        <span
          className={cn(
            "inline-flex h-20 min-w-20 items-center justify-center rounded-pill px-6 text-caption font-medium tabular",
            variant === "primary" ? "bg-on-ink/15 text-on-ink" : variant === "danger" ? "bg-on-danger/20 text-on-danger" : "bg-fill-2 text-fg-secondary",
          )}
        >
          {count}
        </span>
      )}
      {shortcut && <KbdSequence keys={shortcut} tone={variant === "primary" || variant === "danger" ? "onInk" : "default"} />}
      {trailingIcon && <Icon icon={trailingIcon} size={iconSize} />}
      {spinning && !leadingIcon && <Spinner size={spinnerSize} className="absolute inset-0 m-auto" />}
    </>
  );

  const handleClick = (e: MouseEvent<HTMLButtonElement & HTMLAnchorElement>) => {
    if (inert) {
      e.preventDefault();
      return;
    }
    onClick?.(e as MouseEvent<HTMLButtonElement>);
  };

  let el: ReactNode;
  if (href && !disabled) {
    const external = isExternalHref(href);
    el = (
      <LinkComp
        ref={ref as Ref<HTMLAnchorElement>}
        href={href}
        className={classes}
        {...(target ? { target } : external ? { target: "_blank" } : {})}
        {...(rel ? { rel } : external ? { rel: "noopener noreferrer" } : {})}
        {...(domProps as object)}
        {...(busy ? { "aria-busy": true, "data-loading": "" } : {})}
        onClick={handleClick}
      >
        {content}
      </LinkComp>
    );
  } else {
    el = (
      <button
        ref={ref as Ref<HTMLButtonElement>}
        type={type ?? "button"}
        className={classes}
        disabled={disabled && !softDisabled}
        {...(domProps as object)}
        {...(softDisabled ? { "aria-disabled": true } : {})}
        {...(busy ? { "aria-busy": true, "data-loading": "" } : {})}
        {...(softDisabled ? { "data-disabled": "" } : {})}
        onClick={handleClick}
      >
        {content}
      </button>
    );
  }

  if (disabledReason && disabled) {
    return <Tooltip content={disabledReason}>{el as ReactElement}</Tooltip>;
  }
  return el;
});
Button.displayName = "Button";
