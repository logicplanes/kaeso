import { createContext, forwardRef, useContext, type ReactNode } from "react";
import { Tabs as BaseTabs } from "@base-ui/react/tabs";
import { cn } from "../../utils/cn";
import { Icon, type IconData } from "../icon/icon";
import { useInset } from "../../utils/inset";

export const KAESO_TABS_VARIANTS = {
  variant: {
    underline: { classes: "", description: "Underline tabs for detail pages: 40px, a 2px ink indicator that slides" },
    pill: { classes: "", description: "Header tabs: separate 32px pills, the active one filled" },
  },
} as const;
export const KAESO_TABS_DEFAULT_VARIANTS = { variant: "underline" } as const;
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
  const inset = useInset();
  return (
    <BaseTabs.List
      ref={ref}
      className={cn(
        "relative flex min-w-0 items-center overflow-x-auto scrollbar-none",
        variant === "underline" ? "-mx-12 gap-20 px-12 hairline-b-inset-12" : inset ? "-my-4 -mr-4 -ml-16 gap-4 p-4" : "-m-4 gap-4 p-4",
        className,
      )}
      {...props}
    >
      {children}
      {variant === "underline" && (
        <BaseTabs.Indicator className="absolute bottom-0 left-0 h-2 w-(--active-tab-width) translate-x-(--active-tab-left) rounded-pill bg-ink indicator-motion" />
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
        "relative inline-flex shrink-0 cursor-pointer items-center gap-6 text-body font-medium whitespace-nowrap transition-control tap-target disabled:cursor-not-allowed disabled:text-fg-disabled",
        variant === "underline"
          ? "group/tab h-40 text-fg-secondary outline-none hover:text-fg selected:text-fg"
          : "h-32 rounded-pill px-12 text-fg-secondary hover:bg-fill-1 hover:text-fg active:bg-fill-2 selected:bg-fill-2 selected:text-fg focus-ring",
        className,
      )}
    >
      {variant === "underline" ? (
        <span className="-mx-8 inline-flex h-28 items-center gap-6 rounded-pill px-8 outline-2 outline-transparent group-focus-visible/tab:outline-focus">
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

const Panel = forwardRef<HTMLDivElement, { value: string; className?: string | undefined; children: ReactNode; keepMounted?: boolean | undefined }>(
  function TabsPanel({ value, className, children, keepMounted }, ref) {
    return (
      <BaseTabs.Panel ref={ref} value={value} {...(keepMounted ? { keepMounted } : {})} className={cn("outline-none focus-ring-inset", className)}>
        {children}
      </BaseTabs.Panel>
    );
  },
);
Panel.displayName = "Tabs.Panel";

export const Tabs = Object.assign(makeRoot("underline", "Tabs"), { List, Tab, Panel });
export const PillTabs = Object.assign(makeRoot("pill", "PillTabs"), { List, Tab, Panel });
