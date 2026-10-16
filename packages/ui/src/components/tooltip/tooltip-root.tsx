import { forwardRef, useLayoutEffect, type ReactNode } from "react";
import { Tooltip as BaseTooltip } from "@base-ui/react/tooltip";
import { mergeProps } from "@base-ui/react/merge-props";
import { pageBoundary, usePortalContainer } from "../../utils/portal-context.js";
import { cn } from "../../utils/cn.js";
import { KbdSequence } from "../kbd/kbd.js";
import { KAESO_TOOLTIP_DEFAULT_VARIANTS, KAESO_TOOLTIP_VARIANTS, type TooltipProps } from "./tooltip.js";
import type { TriggerProps, TriggerPropsStore } from "./trigger-props.js";

const popupClasses = "z-70 flex items-center gap-8 bg-tooltip text-small font-medium text-fg shadow-raised popup-motion";

export function TooltipRoot({
  content,
  shortcut,
  triggerProps,
  side = KAESO_TOOLTIP_DEFAULT_VARIANTS.side,
  shape = KAESO_TOOLTIP_DEFAULT_VARIANTS.shape,
  open,
  defaultOpen,
  onOpenChange,
  disabled,
  delay,
  sideOffset = 8,
}: TooltipProps & { triggerProps: TriggerPropsStore }) {
  return (
    <BaseTooltip.Root
      {...(open !== undefined ? { open } : {})}
      {...(defaultOpen !== undefined ? { defaultOpen } : {})}
      {...(onOpenChange ? { onOpenChange: (o: boolean) => onOpenChange(o) } : {})}
      {...(disabled !== undefined ? { disabled } : {})}
    >
      <BaseTooltip.Trigger render={(props) => <HandOver store={triggerProps} props={props} />} {...(delay !== undefined ? { delay } : {})} />
      <BaseTooltip.Portal container={usePortalContainer()}>
        <BaseTooltip.Positioner positionMethod="fixed" collisionBoundary={pageBoundary()} side={side} sideOffset={sideOffset} collisionPadding={8}>
          <TooltipPopup shape={shape}>
            <span>{content}</span>
            {shortcut && <KbdSequence keys={shortcut} />}
          </TooltipPopup>
        </BaseTooltip.Positioner>
      </BaseTooltip.Portal>
    </BaseTooltip.Root>
  );
}
TooltipRoot.displayName = "Tooltip.Root";

function HandOver({ store, props }: { store: TriggerPropsStore; props: TriggerProps }) {
  useLayoutEffect(() => store.set(props));
  useLayoutEffect(() => () => store.set(null), [store]);
  return null;
}
HandOver.displayName = "Tooltip.HandOver";

export function mergeTriggerProps(given: TriggerProps, own: object): object {
  const { ref: _ref, ...props } = given;
  return mergeProps<"button">(props, own);
}

const TooltipPopup = forwardRef<HTMLDivElement, { shape: keyof typeof KAESO_TOOLTIP_VARIANTS.shape; children: ReactNode; className?: string | undefined }>(
  function TooltipPopup({ shape, children, className }, ref) {
    return (
      <BaseTooltip.Popup ref={ref} className={cn(popupClasses, KAESO_TOOLTIP_VARIANTS.shape[shape].classes, className)}>
        {children}
      </BaseTooltip.Popup>
    );
  },
);
TooltipPopup.displayName = "Tooltip.Popup";
