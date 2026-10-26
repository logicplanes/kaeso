import { createContext, forwardRef, useContext, useEffect, useMemo, useRef, type ReactNode } from "react";
import { Tabs as BaseTabs } from "@base-ui/react/tabs";
import { cn } from "../../utils/cn.js";
import { mergeRefs } from "../../utils/merge-refs.js";
import { observeResize } from "../../utils/observe-resize.js";
import { Icon, type IconData } from "../icon/icon.js";

export const KAESO_TABS_VARIANTS = {
  variant: {
    underline: {
      classes: "",
      description: "Underline tabs for detail pages: 40px, labels 12px in like every box, a 2px ink indicator under the label that slides",
    },
    pill: { classes: "", description: "Header tabs: separate 32px pills, the active one filled; the first pill's edge on the gutter" },
  },
} as const;
export const KAESO_TABS_DEFAULT_VARIANTS = { variant: "underline" } as const;
export const KAESO_PILL_TABS_VARIANTS = { variant: KAESO_TABS_VARIANTS.variant } as const;
export const KAESO_PILL_TABS_DEFAULT_VARIANTS = { variant: "pill" } as const;
type Variant = keyof typeof KAESO_TABS_VARIANTS.variant;

const Ctx = createContext<Variant>("underline");

export interface TabsProps {
  value?: string | undefined;
  defaultValue?: string | undefined;
  onValueChange?: ((value: string) => void) | undefined;
  variant?: Variant | undefined;
  className?: string | undefined;
  children: ReactNode;
}

function makeRoot(defaultVariant: Variant, name: string) {
  const Root = forwardRef<HTMLDivElement, TabsProps>(function TabsRoot(
    { value, defaultValue, onValueChange, variant = defaultVariant, className, children },
    ref,
  ) {
    return (
      <Ctx.Provider value={variant}>
        <BaseTabs.Root
          ref={ref}
          {...(value !== undefined ? { value } : {})}
          {...(defaultValue !== undefined ? { defaultValue } : {})}
          {...(onValueChange ? { onValueChange: (v: unknown) => onValueChange(String(v)) } : {})}
          className={cn("flex min-w-0 flex-col", className)}
        >
          {children}
        </BaseTabs.Root>
      </Ctx.Provider>
    );
  });
  Root.displayName = name;
  return Root;
}

export interface TabsListProps {
  className?: string | undefined;
  children: ReactNode;
  "aria-label"?: string | undefined;
}

const List = forwardRef<HTMLDivElement, TabsListProps>(function TabsList({ className, children, ...props }, ref) {
  const variant = useContext(Ctx);
  const own = useRef<HTMLDivElement | null>(null);
  const refs = useMemo(() => mergeRefs(own, ref), [ref]);
  useEffect(() => {
    const el = own.current;
    if (!el) return;
    const mark = () => {
      const end = el.scrollWidth - el.clientWidth - Math.abs(el.scrollLeft);
      el.style.setProperty("--scroll-area-overflow-x-start", `${Math.max(0, Math.abs(el.scrollLeft))}px`);
      el.style.setProperty("--scroll-area-overflow-x-end", `${Math.max(0, end)}px`);
    };
    mark();
    el.addEventListener("scroll", mark, { passive: true });
    const stop = observeResize([el, ...el.children], mark);
    return () => {
      el.removeEventListener("scroll", mark);
      stop();
    };
  }, []);
  return (
    <BaseTabs.List
      ref={refs}
      className={cn(
        "relative flex min-w-0 items-center overflow-x-auto scroll-px-16 scrollbar-none scroll-fade-x",
        variant === "underline" ? "hairline-b-strong" : "gap-4",
        className,
      )}
      {...props}
    >
      {children}
      {variant === "underline" && (
        <BaseTabs.Indicator className="absolute bottom-0 left-0 flex h-2 w-(--active-tab-width) translate-x-(--active-tab-left) px-12 indicator-motion">
          <span className="h-2 flex-1 rounded-pill bg-ink forced-line" />
        </BaseTabs.Indicator>
      )}
    </BaseTabs.List>
  );
});
List.displayName = "Tabs.List";

export interface TabProps {
  value: string;
  children: ReactNode;
  icon?: IconData | undefined;
  count?: number | undefined;
  disabled?: boolean | undefined;
  className?: string | undefined;
  "data-force"?: string | undefined;
}

const Tab = forwardRef<HTMLButtonElement, TabProps>(function TabsTab({ value, children, icon, count, disabled, className, "data-force": force }, ref) {
  const variant = useContext(Ctx);
  return (
    <BaseTabs.Tab
      ref={ref}
      value={value}
      {...(disabled ? { disabled: true } : {})}
      {...(force ? { "data-force": force } : {})}
      className={cn(
        "relative inline-flex shrink-0 cursor-pointer items-center gap-6 text-body font-medium whitespace-nowrap text-fg-secondary transition-control tap-target hover:text-fg selected:text-fg disabled:cursor-not-allowed disabled:text-fg-disabled selected:disabled:text-fg-disabled",
        variant === "underline"
          ? "group/tab h-40 outline-none"
          : "h-32 rounded-pill px-12 hover:bg-fill-1 active:bg-fill-2 selected:bg-fill-2 focus-ring-inset",
        className,
      )}
    >
      {variant === "underline" ? (
        <span className="inline-flex h-28 items-center gap-6 rounded-pill px-12 transition-control focus-ring-inset group-hover/tab:bg-fill-1 group-active/tab:bg-fill-2 group-disabled/tab:bg-transparent group-focus-visible/tab:outline-focus group-focus-visible/tab:outline-solid">
          {icon && <Icon icon={icon} size="sm" />}
          {children}
          {count !== undefined && <span className="text-body font-regular text-fg-secondary tabular">{count}</span>}
        </span>
      ) : (
        <>
          {icon && <Icon icon={icon} size="sm" />}
          {children}
          {count !== undefined && <span className="text-body font-regular text-fg-secondary tabular">{count}</span>}
        </>
      )}
    </BaseTabs.Tab>
  );
});
Tab.displayName = "Tabs.Tab";

const Panel = forwardRef<
  HTMLDivElement,
  { value: string; className?: string | undefined; children: ReactNode; keepMounted?: boolean | undefined; "data-force"?: string | undefined }
>(function TabsPanel({ value, className, children, keepMounted, "data-force": force }, ref) {
  return (
    <BaseTabs.Panel
      ref={ref}
      value={value}
      {...(keepMounted ? { keepMounted } : {})}
      {...(force ? { "data-force": force } : {})}
      className={cn("outline-none focus-ring-inset", className)}
    >
      {children}
    </BaseTabs.Panel>
  );
});
Panel.displayName = "Tabs.Panel";

export const Tabs = Object.assign(makeRoot(KAESO_TABS_DEFAULT_VARIANTS.variant, "Tabs"), { List, Tab, Panel });
export const PillTabs = Object.assign(makeRoot(KAESO_PILL_TABS_DEFAULT_VARIANTS.variant, "PillTabs"), { List, Tab, Panel });
