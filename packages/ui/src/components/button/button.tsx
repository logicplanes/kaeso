import { forwardRef, useRef, useState, type ButtonHTMLAttributes, type FocusEvent, type MouseEvent, type ReactElement, type ReactNode, type Ref } from "react";
import { cn } from "../../utils/cn.js";
import { useLinkComponent, isExternalHref } from "../../utils/link-provider.js";
import { warnMissingName, warnStylingClassName } from "../../utils/dev.js";
import { Icon, type IconData, type IconSize } from "../icon/icon.js";
import { Spinner, useDelayedLoading } from "../loader/loader.js";
import { KbdSequence } from "../kbd/kbd.js";
import { Tooltip } from "../tooltip/tooltip.js";
import { SwapText } from "../swap-text/swap-text.js";
import { useTruncated } from "../../hooks/use-truncated.js";

export const KAESO_BUTTON_VARIANTS = {
  variant: {
    primary: { classes: "material-primary", description: "Ink with a faint top light. Exactly one per view." },
    secondary: { classes: "material-secondary", description: "Raised, ring and a soft shadow. The default." },
    ghost: { classes: "material-ghost", description: "Transparent until hovered: toolbars, rows, quiet actions" },
    quiet: {
      classes: "material-ghost text-fg-secondary hover:text-fg active:text-fg open:text-fg",
      description: "A ghost in secondary text that darkens on hover: actions inside a field (clear, reveal, copy)",
    },
    soft: { classes: "material-soft", description: "A fill without a ring: the active pill tab" },
    danger: { classes: "material-danger", description: "The destructive confirm, with the primary's material" },
    "danger-secondary": { classes: "material-danger-secondary", description: "Raised with danger text; tints on hover. Destructive triggers." },
    link: {
      classes:
        "bg-transparent text-fg underline decoration-fg-tertiary decoration-1 underline-offset-3 hover:decoration-current active:decoration-current open:decoration-current disabled:text-fg-disabled",
      description: "Text with an underline, for actions inside sentences",
    },
  },
  size: {
    xs: { classes: "h-24 px-10 gap-4 text-caption font-medium ghost-end:-mr-10 ghost-start:-ml-10", description: "24px: chips inside rows and tables only" },
    sm: { classes: "h-32 px-12 gap-6 text-body font-medium ghost-end:-mr-12 ghost-start:-ml-12", description: "32px: toolbars, headers, settings rows" },
    md: { classes: "h-36 px-16 gap-6 text-body font-medium ghost-end:-mr-16 ghost-start:-ml-16", description: "36px: the default" },
    lg: { classes: "h-40 px-18 gap-8 text-body font-medium ghost-end:-mr-18 ghost-start:-ml-18", description: "40px: prominent actions" },
    xl: {
      classes: "h-48 px-22 gap-8 text-body-lg font-medium ghost-end:-mr-22 ghost-start:-ml-22",
      description: "48px: sign-in and setup forms, empty-state actions",
    },
  },
  shape: {
    pill: { classes: "rounded-pill", description: "Every button is a pill" },
    circle: { classes: "rounded-pill p-0 justify-center ghost-end:mr-0 ghost-start:ml-0", description: "Icon-only: a circle of the same height" },
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

const ARIA_KEY: Record<string, string> = {
  cmd: "Meta",
  ctrl: "Control",
  alt: "Alt",
  option: "Alt",
  shift: "Shift",
  enter: "Enter",
  esc: "Escape",
  backspace: "Backspace",
  tab: "Tab",
  up: "ArrowUp",
  down: "ArrowDown",
  left: "ArrowLeft",
  right: "ArrowRight",
};

// aria-keyshortcuts lists chords only, so a sequence ("G then L") gets none; "mod" is offered as both platforms' chord.
function ariaKeyShortcuts(keys: string[] | string[][]): string | undefined {
  const chord = (Array.isArray(keys[0]) ? (keys.length === 1 ? keys[0] : []) : keys) as string[];
  if (!chord.length) return undefined;
  const join = (mod: string) =>
    chord.map((k) => (k.toLowerCase() === "mod" ? mod : (ARIA_KEY[k.toLowerCase()] ?? (k.length === 1 ? k.toUpperCase() : k)))).join("+");
  return chord.some((k) => k.toLowerCase() === "mod") ? `${join("Meta")} ${join("Control")}` : join("");
}

export function buttonClasses({
  variant = KAESO_BUTTON_DEFAULT_VARIANTS.variant,
  size = KAESO_BUTTON_DEFAULT_VARIANTS.size,
  shape = KAESO_BUTTON_DEFAULT_VARIANTS.shape,
  leading = false,
  trailing = false,
  fullWidth = false,
}: {
  variant?: ButtonVariant | undefined;
  size?: ButtonSize | undefined;
  shape?: ButtonShape | undefined;
  leading?: boolean | undefined;
  trailing?: boolean | undefined;
  fullWidth?: boolean | undefined;
} = {}) {
  return cn(
    "relative inline-flex max-w-full shrink-0 select-none items-center justify-center whitespace-nowrap no-underline",
    "cursor-pointer transition-control focus-ring tap-target disabled:cursor-not-allowed",
    KAESO_BUTTON_VARIANTS.size[size].classes,
    KAESO_BUTTON_VARIANTS.shape[shape].classes,
    shape === "circle" && CIRCLE[size],
    shape === "pill" && leading && variant !== "link" && LEADING_ICON_PADDING[size],
    shape === "pill" && trailing && variant !== "link" && TRAILING_ICON_PADDING[size],
    variant === "link" && "-mx-2 h-auto rounded-code px-2 align-baseline outline-offset-0",
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
    onFocus,
    onBlur,
    ...rest
  } = props;
  const LinkComp = useLinkComponent();
  const [focused, setFocused] = useState(false);
  const labelRef = useRef<HTMLSpanElement>(null);
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

  // A natively disabled button that holds focus drops it to <body>; stay focusable until it blurs.
  const softDisabled = (!!disabledReason || focused) && disabled;
  const cut = useTruncated(labelRef, !isCircle);
  const describe = !!disabledReason && disabled && disabledReason !== (props as { "aria-label"?: string | undefined })["aria-label"];
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
    size === "xs" && count !== undefined && !trailingIcon && !shortcut && "pr-2",
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
      <span ref={labelRef} className={cn("min-w-0 truncate", spinning && !leadingIcon && "opacity-0")}>
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
      {shortcut && (
        <KbdSequence
          aria-hidden
          data-shortcut=""
          keys={shortcut}
          tone={variant === "primary" || variant === "danger" ? "onInk" : "default"}
          className={cn(variant === "danger" && "kbd-on-danger")}
        />
      )}
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

  const handleFocus = (e: FocusEvent<HTMLButtonElement & HTMLAnchorElement>) => {
    setFocused(true);
    onFocus?.(e as FocusEvent<HTMLButtonElement>);
  };
  const handleBlur = (e: FocusEvent<HTMLButtonElement & HTMLAnchorElement>) => {
    setFocused(false);
    onBlur?.(e as FocusEvent<HTMLButtonElement>);
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
        {...(shortcut ? { "aria-keyshortcuts": ariaKeyShortcuts(shortcut) } : {})}
        {...(busy ? { "aria-busy": true, "data-loading": "" } : {})}
        onClick={handleClick}
        onFocus={handleFocus}
        onBlur={handleBlur}
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
        {...(shortcut ? { "aria-keyshortcuts": ariaKeyShortcuts(shortcut) } : {})}
        {...(softDisabled ? { "aria-disabled": true } : {})}
        {...(busy ? { "aria-busy": true, "data-loading": "" } : {})}
        {...(softDisabled ? { "data-disabled": "" } : {})}
        {...(describe ? { "aria-description": disabledReason } : {})}
        onClick={handleClick}
        onFocus={handleFocus}
        onBlur={handleBlur}
      >
        {content}
      </button>
    );
  }

  if (disabledReason && disabled) {
    return <Tooltip content={disabledReason}>{el as ReactElement}</Tooltip>;
  }
  if (cut) {
    return <Tooltip content={(props as PillButtonProps).children}>{el as ReactElement}</Tooltip>;
  }
  return el;
});
Button.displayName = "Button";
