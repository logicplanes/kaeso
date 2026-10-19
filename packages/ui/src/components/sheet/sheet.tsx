import { InsetContent } from "../../utils/inset.js";
import { usePortalContainer } from "../../utils/portal-context.js";
import { forwardRef, useCallback, useState, type ReactElement, type ReactNode } from "react";
import { Dialog as BaseDialog } from "@base-ui/react/dialog";
import { cn } from "../../utils/cn.js";
import { useRestoreFocus } from "../../utils/restore-focus.js";
import { useScopedDialog } from "../../utils/dialog-scope.js";
import { icons } from "../../icons/vocabulary.js";
import { IconButton } from "../icon-button/icon-button.js";
import { useUiText } from "../../i18n/context.js";
import { useResizeMotion } from "../../utils/resize-motion.js";

export const KAESO_SHEET_VARIANTS = {
  size: {
    sm: { classes: "w-480", description: "480px (default)" },
    md: { classes: "w-640", description: "640px: the request inspector" },
    lg: { classes: "w-800", description: "800px" },
  },
  side: {
    right: { classes: "top-8 right-8 bottom-8 sheet-limits-x", description: "From the right (default)" },
    left: { classes: "top-8 bottom-8 left-8 sheet-limits-x", description: "From the left: the mobile sidebar" },
    bottom: { classes: "right-8 bottom-8 left-8 sheet-limits-y", description: "From the bottom: mobile menus" },
  },
} as const;
export const KAESO_SHEET_DEFAULT_VARIANTS = { size: "sm", side: "right" } as const;

export interface SheetProps {
  open?: boolean | undefined;
  defaultOpen?: boolean | undefined;
  onOpenChange?: ((open: boolean) => void) | undefined;
  trigger?: ReactElement | undefined;
  title: ReactNode;
  subtitle?: ReactNode | undefined;
  actions?: ReactNode | undefined;
  size?: keyof typeof KAESO_SHEET_VARIANTS.size | undefined;
  side?: keyof typeof KAESO_SHEET_VARIANTS.side | undefined;
  modal?: boolean | undefined;
  children: ReactNode;
  footer?: ReactNode | undefined;
  inline?: boolean | undefined;
}

function Frame({ title, subtitle, actions, children, footer, inline, onOpenChange, side }: SheetProps) {
  const Title = inline ? "h2" : BaseDialog.Title;
  const t = useUiText();
  const [box, setBox] = useState<HTMLElement | null>(null);
  const grows = side === "bottom";
  const headRef = useCallback((el: HTMLElement | null) => setBox(grows ? (el?.parentElement ?? null) : null), [grows]);
  useResizeMotion(box);
  const close = (
    <IconButton
      icon={icons.close}
      aria-label={t("sheet.close", "Close")}
      tooltip={false}
      {...(inline && onOpenChange ? { onClick: () => onOpenChange(false) } : {})}
    />
  );
  return (
    <>
      <div ref={headRef} className="flex h-60 shrink-0 items-center gap-12 px-24 hairline-b">
        <div className="flex min-w-0 flex-1 flex-col">
          <Title className="truncate text-title-sm font-semibold text-fg">{title}</Title>
          {subtitle && <span className="truncate mono text-small text-fg-secondary">{subtitle}</span>}
        </div>
        {actions}
        {inline ? close : <BaseDialog.Close render={close} />}
      </div>
      <div data-dialog-body="" className="min-h-0 flex-1 overflow-y-auto overscroll-contain px-24 py-20 scrollbar-quiet">
        {children}
      </div>
      {footer && <div className="flex shrink-0 flex-wrap items-center justify-end gap-8 px-24 py-16 hairline-t">{footer}</div>}
    </>
  );
}

export const Sheet = forwardRef<HTMLDivElement, SheetProps>(function Sheet(props, ref) {
  const { open, defaultOpen, onOpenChange, trigger, size = "sm", side = "right", modal = true, inline } = props;
  const box = cn(
    "flex flex-col overflow-hidden rounded-sheet bg-raised text-fg shadow-dialog outline-none",
    side !== "bottom" && KAESO_SHEET_VARIANTS.size[size].classes,
  );
  const focus = useRestoreFocus(open, () => modal);
  const container = usePortalContainer();
  const settled = useScopedDialog(inline ? false : open);
  if (inline)
    return (
      <div ref={ref} className={cn(box, "h-560")}>
        <Frame {...props} />
      </div>
    );
  return (
    <BaseDialog.Root
      {...(open !== undefined ? { open } : {})}
      {...(defaultOpen !== undefined ? { defaultOpen } : {})}
      {...(onOpenChange ? { onOpenChange: (o: boolean) => onOpenChange(o) } : {})}
      modal={modal}
      onOpenChangeComplete={settled}
    >
      {trigger && <BaseDialog.Trigger render={trigger} />}
      <BaseDialog.Portal container={container}>
        {modal && <BaseDialog.Backdrop className="fixed inset-0 z-40 bg-overlay opacity-50 backdrop-motion" />}
        <BaseDialog.Popup
          ref={ref}
          data-side={side}
          initialFocus={focus.initialFocus}
          finalFocus={focus.finalFocus}
          className={cn("fixed z-40 sheet-motion", box, KAESO_SHEET_VARIANTS.side[side].classes, !modal && side !== "bottom" && "sheet-inspector")}
        >
          <InsetContent inset={false}>
            <Frame {...props} />
          </InsetContent>
        </BaseDialog.Popup>
      </BaseDialog.Portal>
    </BaseDialog.Root>
  );
});
Sheet.displayName = "Sheet";
