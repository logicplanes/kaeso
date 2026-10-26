import { forwardRef, useState, useSyncExternalStore, type ButtonHTMLAttributes, type ReactElement, type ReactNode } from "react";
import { Popover as BasePopover } from "@base-ui/react/popover";
import { Dialog as BaseDialog } from "@base-ui/react/dialog";
import { cn } from "../../utils/cn.js";
import { pageBoundary, usePortalContainer } from "../../utils/portal-context.js";
import { icons } from "../../icons/vocabulary.js";
import { Icon } from "../icon/icon.js";
import { IconButton } from "../icon-button/icon-button.js";
import { KAESO_SELECT_VARIANTS } from "../select/select.js";
import { useUiText } from "../../i18n/context.js";
import { joinIds, showsText, useHiddenDescription } from "../../utils/hidden-description.js";
import { CommandListHeightContext } from "./command-list.js";

export const KAESO_PICKER_POPOVER_VARIANTS = {
  width: {
    sm: { classes: "w-320", description: "320px: providers and short lists" },
    md: { classes: "w-400", description: "400px (default)" },
    lg: { classes: "w-560", description: "560px: models with their details" },
  },
} as const;
export const KAESO_PICKER_POPOVER_DEFAULT_VARIANTS = { width: "md" } as const;

const NARROW = "(max-width: 767px)";
const subscribeNarrow = (onChange: () => void) => {
  if (typeof window === "undefined" || !window.matchMedia) return () => {};
  const q = window.matchMedia(NARROW);
  q.addEventListener("change", onChange);
  return () => q.removeEventListener("change", onChange);
};
const isNarrow = () => typeof window !== "undefined" && !!window.matchMedia?.(NARROW).matches;

export interface PickerPopoverProps {
  trigger: ReactElement;
  title: string;
  open?: boolean | undefined;
  onOpenChange?: ((open: boolean) => void) | undefined;
  width?: keyof typeof KAESO_PICKER_POPOVER_VARIANTS.width | undefined;
  align?: "start" | "end" | undefined;
  children: ReactNode;
}

export const PickerPopover = forwardRef<HTMLDivElement, PickerPopoverProps>(function PickerPopover(
  { trigger, title, open, onOpenChange, width = "md", align = "start", children },
  ref,
) {
  const t = useUiText();
  const container = usePortalContainer();
  const narrow = useSyncExternalStore(subscribeNarrow, isNarrow, () => false);
  const change = onOpenChange ? { onOpenChange: (o: boolean) => onOpenChange(o) } : {};
  const state = open !== undefined ? { open } : {};
  if (narrow)
    return (
      <BaseDialog.Root {...state} {...change}>
        <BaseDialog.Trigger render={trigger} />
        <BaseDialog.Portal container={container}>
          <BaseDialog.Backdrop className="fixed inset-0 z-50 bg-overlay opacity-50 backdrop-motion" />
          <BaseDialog.Popup
            ref={ref}
            data-side="bottom"
            className="fixed inset-8 z-50 flex flex-col overflow-hidden rounded-sheet bg-raised text-fg shadow-dialog outline-none sheet-motion"
          >
            <div className="flex h-56 shrink-0 items-center gap-12 pr-8 pl-20 hairline-b">
              <BaseDialog.Title className="min-w-0 flex-1 truncate text-body-lg font-semibold text-fg">{title}</BaseDialog.Title>
              <BaseDialog.Close render={<IconButton icon={icons.close} aria-label={t("pickerPopover.close", "Close")} tooltip={false} />} />
            </div>
            <CommandListHeightContext.Provider value="fill">{children}</CommandListHeightContext.Provider>
          </BaseDialog.Popup>
        </BaseDialog.Portal>
      </BaseDialog.Root>
    );
  return (
    <BasePopover.Root {...state} {...change} modal={false}>
      <BasePopover.Trigger render={trigger} />
      <BasePopover.Portal container={container}>
        <BasePopover.Positioner
          positionMethod="fixed"
          collisionBoundary={pageBoundary()}
          side="bottom"
          align={align}
          sideOffset={6}
          collisionPadding={8}
          className="z-50 outline-none"
        >
          <BasePopover.Popup
            ref={ref}
            aria-label={title}
            className={cn(
              "flex max-w-full-bleed-4 flex-col overflow-hidden rounded-popover bg-raised text-fg shadow-raised outline-none popup-motion",
              KAESO_PICKER_POPOVER_VARIANTS.width[width].classes,
            )}
          >
            <CommandListHeightContext.Provider value="popover">{children}</CommandListHeightContext.Provider>
          </BasePopover.Popup>
        </BasePopover.Positioner>
      </BasePopover.Portal>
    </BasePopover.Root>
  );
});
PickerPopover.displayName = "PickerPopover";

export interface PickerTriggerProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children"> {
  appearance?: keyof typeof KAESO_SELECT_VARIANTS.appearance | undefined;
  size?: keyof typeof KAESO_SELECT_VARIANTS.size | undefined;
  fullWidth?: boolean | undefined;
  invalid?: boolean | undefined;
  placeholder?: ReactNode | undefined;
  children?: ReactNode | undefined;
  "aria-label"?: string | undefined;
  "data-force"?: string | undefined;
}

export const PickerTrigger = forwardRef<HTMLButtonElement, PickerTriggerProps>(function PickerTrigger(
  {
    appearance = "default",
    size = "md",
    fullWidth,
    invalid,
    placeholder,
    children,
    className,
    type = "button",
    "aria-label": ariaLabel,
    "aria-describedby": describedBy,
    disabled,
    onFocus,
    onBlur,
    onClickCapture,
    ...props
  },
  ref,
) {
  const [focused, setFocused] = useState(false);
  // A natively disabled button that holds focus drops it to <body>; stay focusable until it blurs.
  const held = !!disabled && focused;
  const empty = children === undefined || children === null || children === false;
  const shows = showsText(empty ? placeholder : children);
  const purpose = useHiddenDescription(shows ? ariaLabel : undefined);
  const descriptions = joinIds(purpose.id, describedBy);
  return (
    <>
      <button
        ref={ref}
        type={type}
        data-picker=""
        {...props}
        {...(held ? { "aria-disabled": true, "data-disabled": "" } : { disabled })}
        onFocus={(e) => {
          setFocused(true);
          onFocus?.(e);
        }}
        onBlur={(e) => {
          setFocused(false);
          onBlur?.(e);
        }}
        onClickCapture={(e) => {
          if (held) {
            e.preventDefault();
            e.stopPropagation();
          }
          onClickCapture?.(e);
        }}
        {...(!shows && ariaLabel ? { "aria-label": ariaLabel } : {})}
        {...(descriptions ? { "aria-describedby": descriptions } : {})}
        {...(invalid ? { "aria-invalid": true } : {})}
        className={cn(
          "group/picker inline-flex min-w-0 cursor-pointer items-center justify-between rounded-pill font-medium whitespace-nowrap transition-control disabled:cursor-not-allowed",
          KAESO_SELECT_VARIANTS.size[size].classes,
          KAESO_SELECT_VARIANTS.appearance[appearance].classes,
          invalid && appearance !== "default" && "ring-invalid",
          fullWidth && "w-full",
          className,
        )}
      >
        <span
          className={cn(
            "flex min-w-0 flex-1 items-center truncate text-left group-disabled/picker:opacity-40",
            empty ? "font-regular text-fg-tertiary" : "group-disabled/picker:text-fg",
          )}
        >
          {empty ? placeholder : children}
        </span>
        <span className="flex shrink-0 text-fg-tertiary group-disabled/picker:opacity-40">
          <Icon icon={icons.chevronDown} size="sm" />
        </span>
      </button>
      {purpose.node}
    </>
  );
});
PickerTrigger.displayName = "PickerTrigger";
