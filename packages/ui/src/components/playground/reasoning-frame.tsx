import { forwardRef, type ReactNode } from "react";
import { Collapsible as BaseCollapsible } from "@base-ui/react/collapsible";
import { cn } from "../../utils/cn.js";
import { icons } from "../../icons/vocabulary.js";
import { Icon } from "../icon/icon.js";

export interface ReasoningFrameProps {
  heading: ReactNode;
  preview?: ReactNode | undefined;
  previewHidden?: boolean | undefined;
  live?: boolean | undefined;
  open?: boolean | undefined;
  defaultOpen?: boolean | undefined;
  onOpenChange?: ((open: boolean) => void) | undefined;
  children: ReactNode;
  className?: string | undefined;
  "data-force"?: string | undefined;
}

export const ReasoningFrame = forwardRef<HTMLDivElement, ReasoningFrameProps>(function ReasoningFrame(
  { heading, preview, previewHidden = false, live = false, open, defaultOpen, onOpenChange, children, className, "data-force": force },
  ref,
) {
  return (
    <BaseCollapsible.Root
      ref={ref}
      {...(open !== undefined ? { open } : {})}
      {...(defaultOpen !== undefined ? { defaultOpen } : {})}
      {...(onOpenChange ? { onOpenChange: (o: boolean) => onOpenChange(o) } : {})}
      className={cn("flex min-w-0 flex-col", className)}
    >
      <BaseCollapsible.Trigger
        {...(force ? { "data-force": force } : {})}
        className="group/think flex h-32 min-w-0 cursor-pointer items-center gap-6 rounded-pill px-12 text-left text-small text-fg-secondary transition-control hover:bg-fill-1 hover:text-fg active:bg-fill-2 focus-ring-inset coarse:h-44"
      >
        <Icon
          icon={icons.chevronRight}
          size="xs"
          className="shrink-0 text-fg-tertiary transition-transform duration-180 ease-overlay group-hover/think:text-fg-secondary group-data-panel-open/think:rotate-90"
        />
        <span className={cn("shrink-0 font-medium", live && "animate-pulse-quiet reduced:animate-none")}>{heading}</span>
        <span aria-hidden={previewHidden || undefined} className="min-w-0 flex-1 truncate text-fg-tertiary group-data-panel-open/think:invisible">
          {preview}
        </span>
      </BaseCollapsible.Trigger>
      <BaseCollapsible.Panel className="collapse-motion">
        <div className="relative pt-4 pb-8 pl-32">
          <span aria-hidden className="absolute top-4 left-18 bottom-8 w-1 rounded-pill bg-line forced-line" />
          {children}
        </div>
      </BaseCollapsible.Panel>
    </BaseCollapsible.Root>
  );
});
ReasoningFrame.displayName = "ReasoningFrame";
