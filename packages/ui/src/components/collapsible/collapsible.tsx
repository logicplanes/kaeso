import { forwardRef, type ReactNode } from "react";
import { Collapsible as BaseCollapsible } from "@base-ui/react/collapsible";
import { Accordion as BaseAccordion } from "@base-ui/react/accordion";
import { cn } from "../../utils/cn.js";
import { icons } from "../../icons/vocabulary.js";
import { Icon } from "../icon/icon.js";

export const KAESO_COLLAPSIBLE_VARIANTS = {
  state: {
    closed: { classes: "", description: "Closed: the chevron points to the row (default)" },
    open: { classes: "group-data-panel-open/dt:rotate-90", description: "Open: the chevron turns down and the content shows" },
  },
} as const;
export const KAESO_COLLAPSIBLE_DEFAULT_VARIANTS = { state: "closed" } as const;

const TRIGGER =
  "group/dt flex min-h-40 w-full cursor-pointer items-center gap-8 rounded-item px-12 text-left text-body font-medium text-fg transition-control hover:bg-fill-1 focus-ring-inset";

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
        <Icon
          icon={icons.chevronRight}
          size="sm"
          className={cn("text-fg-tertiary transition-transform duration-180 ease-overlay", KAESO_COLLAPSIBLE_VARIANTS.state.open.classes)}
        />
        <span className="min-w-0 flex-1">{title}</span>
        {hint && <span className="text-small font-regular text-fg-secondary">{hint}</span>}
      </BaseCollapsible.Trigger>
      <BaseCollapsible.Panel className="collapse-motion">
        <div className="pt-4 pr-12 pb-12 pl-36">{children}</div>
      </BaseCollapsible.Panel>
    </BaseCollapsible.Root>
  );
});
Collapsible.displayName = "Collapsible";

export interface AccordionItem {
  value: string;
  title: ReactNode;
  content: ReactNode;
  hint?: ReactNode | undefined;
}

export interface AccordionProps {
  items: AccordionItem[];
  multiple?: boolean | undefined;
  defaultValue?: string[] | undefined;
  className?: string | undefined;
}

export const Accordion = forwardRef<HTMLDivElement, AccordionProps>(function Accordion({ items, multiple = false, defaultValue, className }, ref) {
  return (
    <BaseAccordion.Root
      ref={ref}
      {...(multiple ? { multiple: true } : {})}
      {...(defaultValue ? { defaultValue } : {})}
      className={cn("flex flex-col", className)}
    >
      {items.map((it) => (
        <BaseAccordion.Item key={it.value} value={it.value} className="hairline-b">
          <BaseAccordion.Header>
            <BaseAccordion.Trigger className={cn(TRIGGER, "min-h-48")}>
              <span className="min-w-0 flex-1">{it.title}</span>
              {it.hint && <span className="text-small font-regular text-fg-secondary">{it.hint}</span>}
              <Icon
                icon={icons.chevronDown}
                size="sm"
                className="text-fg-tertiary transition-transform duration-180 ease-overlay group-data-panel-open/dt:rotate-180"
              />
            </BaseAccordion.Trigger>
          </BaseAccordion.Header>
          <BaseAccordion.Panel className="collapse-motion">
            <div className="px-12 pb-16 text-body text-fg-secondary">{it.content}</div>
          </BaseAccordion.Panel>
        </BaseAccordion.Item>
      ))}
    </BaseAccordion.Root>
  );
});
Accordion.displayName = "Accordion";
