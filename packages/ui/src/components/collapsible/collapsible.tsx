import { forwardRef, type ReactNode } from "react";
import { Collapsible as BaseCollapsible } from "@base-ui/react/collapsible";
import { cn } from "../../utils/cn.js";
import { DISCLOSURE_TRIGGER, DisclosureChevron } from "./disclosure.js";

export const KAESO_COLLAPSIBLE_VARIANTS = {
  state: {
    closed: { classes: "", description: "Closed: a pill row with the title 12px in and a chevron at its end pointing right (default)" },
    open: { classes: "group-data-panel-open/dt:rotate-90", description: "Open: the chevron turns down in 200ms and the content shows under the title" },
  },
} as const;
export const KAESO_COLLAPSIBLE_DEFAULT_VARIANTS = { state: "closed" } as const;

const TRIGGER = `${DISCLOSURE_TRIGGER} min-h-40 py-8 text-body font-medium text-fg hover:bg-fill-1 active:bg-fill-2 coarse:min-h-44`;

function DisclosureText({ title, hint }: { title: ReactNode; hint: ReactNode | undefined }) {
  return (
    <span className="flex min-w-0 flex-1 items-baseline gap-8">
      <span className="min-w-0 flex-1">{title}</span>
      {hint && <span className="shrink-0 text-small font-regular whitespace-nowrap text-fg-secondary tabular-nums">{hint}</span>}
    </span>
  );
}

export interface CollapsibleProps {
  title: ReactNode;
  children: ReactNode;
  open?: boolean | undefined;
  defaultOpen?: boolean | undefined;
  onOpenChange?: ((open: boolean) => void) | undefined;
  hint?: ReactNode | undefined;
  className?: string | undefined;
}

export const Collapsible = forwardRef<HTMLDivElement, CollapsibleProps>(function Collapsible(
  { title, children, open, defaultOpen, onOpenChange, hint, className },
  ref,
) {
  return (
    <BaseCollapsible.Root
      ref={ref}
      {...(open !== undefined ? { open } : {})}
      {...(defaultOpen !== undefined ? { defaultOpen } : {})}
      {...(onOpenChange ? { onOpenChange: (o: boolean) => onOpenChange(o) } : {})}
      className={cn("flex flex-col", className)}
    >
      <BaseCollapsible.Trigger className={TRIGGER}>
        <DisclosureText title={title} hint={hint} />
        <DisclosureChevron />
      </BaseCollapsible.Trigger>
      <BaseCollapsible.Panel className="collapse-motion">
        <div className="px-12 pt-4 pb-12">{children}</div>
      </BaseCollapsible.Panel>
    </BaseCollapsible.Root>
  );
});
Collapsible.displayName = "Collapsible";
