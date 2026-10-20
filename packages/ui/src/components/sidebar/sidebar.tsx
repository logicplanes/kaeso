import type * as React from "react";
import {
  createContext,
  forwardRef,
  useCallback,
  useContext,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type HTMLAttributes,
  type MouseEvent,
  type ReactElement,
  type ReactNode,
} from "react";
import { Collapsible } from "@base-ui/react/collapsible";
import { cn } from "../../utils/cn.js";
import { observeResize } from "../../utils/observe-resize.js";
import { useLinkComponent } from "../../utils/link-provider.js";
import { useStoredState } from "../../hooks/use-stored-state.js";
import { Icon, type IconData } from "../icon/icon.js";
import { icons } from "../../icons/vocabulary.js";
import { Tooltip } from "../tooltip/tooltip.js";
import { Skeleton } from "../skeleton/skeleton.js";
import { NavWordmark } from "../logo/nav-wordmark.js";
import { lazyModule, useLazyModule } from "../../utils/lazy-module.js";
import { whenIdle } from "../../utils/when-idle.js";
import { HandedTrigger, triggerPropsStore } from "../tooltip/trigger-props.js";
import { useUiText } from "../../i18n/context.js";

const strong = (text: string) =>
  text.split("**").map((part, i) =>
    i % 2 ? (
      <span key={i} className="font-semibold text-fg">
        {part}
      </span>
    ) : (
      part
    ),
  );

export const KAESO_SIDEBAR_VARIANTS = {
  mode: {
    expanded: { classes: "w-(--k-sidebar-width)", description: "248px by default, resizable between 232 and 280" },
    rail: { classes: "w-60", description: "60px: icons only, labels in tooltips" },
  },
  item: {
    default: { classes: "text-fg hover:bg-fill-2 active:bg-fill-3", description: "A navigation item" },
    active: { classes: "bg-fill-3 text-fg", description: "The current page" },
  },
  caption: {
    secondary: { classes: "text-fg-secondary", description: "A quiet second line under the switcher's label (default)" },
    danger: { classes: "text-danger-text", description: "A second line that warns: the organization is scheduled for deletion" },
  },
} as const;
export const KAESO_SIDEBAR_DEFAULT_VARIANTS = { mode: "expanded", item: "default", caption: "secondary" } as const;

interface SidebarCtx {
  rail: boolean;
  setRail: (v: boolean) => void;
  collapsed: boolean;
  setCollapsed: (v: boolean) => void;
  peeking: boolean;
  setPeeking: (v: boolean) => void;
  width: number;
  setWidth: (v: number) => void;
  previewWidth: (v: number | null) => void;
  storageKey: string | undefined;
}
const Ctx = createContext<SidebarCtx>({
  rail: false,
  setRail: () => undefined,
  collapsed: false,
  setCollapsed: () => undefined,
  peeking: false,
  setPeeking: () => undefined,
  width: 248,
  setWidth: () => undefined,
  previewWidth: () => undefined,
  storageKey: undefined,
});
export const useSidebar = () => useContext(Ctx);

export const SidebarFloatingContext = createContext(false);
export const SidebarSheetContext = createContext<{ close: () => void; peek: boolean } | null>(null);

export interface SidebarProviderProps {
  children: ReactNode;
  storageKey?: string | undefined;
  defaultRail?: boolean | undefined;
  shortcut?: boolean | undefined;
}

function Provider({ children, storageKey, defaultRail = false, shortcut = true }: SidebarProviderProps) {
  const [rail, setRail] = useStoredState(storageKey ? `${storageKey}:rail` : undefined, defaultRail);
  const [storedWidth, setWidth] = useStoredState(storageKey ? `${storageKey}:width` : undefined, 248);
  const [preview, previewWidth] = useState<number | null>(null);
  const width = clampWidth(preview ?? storedWidth);
  const [collapsed, setCollapsedState] = useStoredState(storageKey ? `${storageKey}:collapsed` : undefined, false);
  const [peeking, setPeeking] = useState(false);
  const setCollapsed = useCallback(
    (v: boolean) => {
      const leaving = v && !!document.activeElement?.closest("[data-sidebar-root]");
      setCollapsedState(v);
      setPeeking(false);
      if (leaving) requestAnimationFrame(() => document.querySelector<HTMLElement>("[data-sidebar-expand]")?.focus());
    },
    [setCollapsedState],
  );
  useEffect(() => {
    if (!shortcut) return;
    const onKey = (e: KeyboardEvent) => {
      const t = e.target;
      if (e.key !== "[" || e.metaKey || e.ctrlKey || e.altKey || (t instanceof Element && t.closest("input, textarea, select, [contenteditable=true]"))) return;
      setCollapsed(!collapsed);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [collapsed, setCollapsed, shortcut]);
  const value = useMemo(
    () => ({ rail, setRail, collapsed, setCollapsed, peeking, setPeeking, width, setWidth, previewWidth, storageKey }),
    [rail, setRail, collapsed, setCollapsed, peeking, width, setWidth, storageKey],
  );
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}
Provider.displayName = "Sidebar.Provider";

export interface SidebarProps extends HTMLAttributes<HTMLElement> {
  resizable?: boolean | undefined;
  "aria-label"?: string | undefined;
}

export const SIDEBAR_MIN = 232;
export const SIDEBAR_MAX = 280;

const clampWidth = (v: number) => Math.round(Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, Number.isFinite(v) ? v : 248)));

const Root = forwardRef<HTMLElement, SidebarProps>(function Sidebar({ resizable = true, className, children, style, "aria-label": ariaLabel, ...props }, ref) {
  const { rail, width, setWidth, previewWidth, collapsed, setCollapsed } = useSidebar();
  const t = useUiText();
  const [hint, setHint] = useState<number | null>(null);
  const [dragging, setDragging] = useState(false);
  const endDrag = useRef<(() => void) | null>(null);
  useEffect(() => () => endDrag.current?.(), []);
  const onPointerDown = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      if (e.button !== 0) return;
      e.preventDefault();
      endDrag.current?.();
      const handle = e.currentTarget;
      const startX = e.clientX;
      const startW = width;
      let moved = false;
      let last = startW;
      const move = (ev: PointerEvent) => {
        if (Math.abs(ev.clientX - startX) > 3) {
          moved = true;
          setDragging(true);
        }
        if (!moved) return;
        last = clampWidth(startW + ev.clientX - startX);
        previewWidth(last);
      };
      const end = () => {
        handle.removeEventListener("pointermove", move);
        handle.removeEventListener("pointerup", up);
        handle.removeEventListener("pointercancel", end);
        document.body.style.cursor = "";
        document.body.style.userSelect = "";
        endDrag.current = null;
        setDragging(false);
        if (moved) setWidth(last);
        previewWidth(null);
      };
      const up = () => {
        end();
        if (!moved) setCollapsed(true);
      };
      handle.setPointerCapture(e.pointerId);
      handle.addEventListener("pointermove", move);
      handle.addEventListener("pointerup", up);
      handle.addEventListener("pointercancel", end);
      document.body.style.cursor = "col-resize";
      document.body.style.userSelect = "none";
      endDrag.current = end;
    },
    [width, setWidth, previewWidth, setCollapsed],
  );
  const floating = useContext(SidebarFloatingContext);
  const hidden = collapsed && !floating;
  const nav = (
    <nav
      ref={ref}
      aria-label={ariaLabel ?? t("sidebar.main", "Main")}
      data-rail={rail || undefined}
      data-sidebar-root=""
      className={cn(
        "group/sidebar relative flex h-full shrink-0 flex-col bg-chrome text-fg",
        rail ? KAESO_SIDEBAR_VARIANTS.mode.rail.classes : KAESO_SIDEBAR_VARIANTS.mode.expanded.classes,
        className,
      )}
      style={{ ["--k-sidebar-width" as string]: `${width}px`, ...style }}
      {...props}
    >
      {children}
    </nav>
  );
  if (floating) return nav;
  return (
    <div className="relative flex h-full shrink-0">
      <div
        className="sidebar-motion relative h-full shrink-0"
        style={{ width: hidden ? 8 : rail ? 60 : width }}
        {...(hidden ? { "data-collapsed": "", inert: true, "aria-hidden": true } : {})}
      >
        <div className="sidebar-content-motion h-full" style={{ width: rail ? 60 : width }}>
          {nav}
        </div>
      </div>
      {resizable && !rail && !hidden && (
        <div
          role="separator"
          aria-orientation="vertical"
          aria-label={t("sidebar.resizeOrCollapse", "Resize sidebar; click to collapse")}
          aria-valuemin={SIDEBAR_MIN}
          aria-valuemax={SIDEBAR_MAX}
          aria-valuenow={width}
          tabIndex={0}
          onPointerDown={onPointerDown}
          onPointerMove={(e) => setHint(Math.round(e.clientY - e.currentTarget.getBoundingClientRect().top))}
          onPointerLeave={() => setHint(null)}
          onKeyDown={(e) => {
            if (e.key === "ArrowLeft") setWidth(clampWidth(width - 8));
            if (e.key === "ArrowRight") setWidth(clampWidth(width + 8));
            if (e.key === "Enter" || e.key === " ") setCollapsed(true);
          }}
          className="absolute top-0 right-0 z-10 h-full w-8 cursor-col-resize rounded-pill focus-ring-inset"
        >
          {hint !== null && !dragging && (
            <span
              aria-hidden
              className="pointer-events-none absolute left-16 flex w-max flex-col gap-2 rounded-tooltip bg-tooltip px-12 py-8 text-small text-fg-secondary shadow-raised"
              style={{ top: Math.max(8, (hint ?? 0) - 24) }}
            >
              <span>{strong(t("sidebar.dragToResize", "**Drag** to resize"))}</span>
              <span>{strong(t("sidebar.clickToCollapse", "**Click** to collapse"))}</span>
            </span>
          )}
        </div>
      )}
    </div>
  );
});
Root.displayName = "Sidebar";

const Header = forwardRef<HTMLDivElement, HTMLAttributes<HTMLDivElement>>(function SidebarHeader({ className, ...props }, ref) {
  return <div ref={ref} className={cn("mt-8 flex h-52 shrink-0 items-center gap-4 px-8", className)} {...props} />;
});
Header.displayName = "Sidebar.Header";

export interface SidebarBrandProps {
  href?: string | undefined;
  label?: string | undefined;
  children?: ReactNode | undefined;
  className?: string | undefined;
}

const Brand = forwardRef<HTMLDivElement, SidebarBrandProps>(function SidebarBrand({ href, label = "Kaeso", children, className }, ref) {
  const { rail } = useSidebar();
  if (rail) {
    return (
      <Header ref={ref} className={cn("justify-center px-0", className)}>
        {children}
      </Header>
    );
  }
  return (
    <Header ref={ref} className={cn("justify-between pr-16 pl-28", className)}>
      <BrandWordmark href={href} label={label} />
      {children && <span className="flex items-center">{children}</span>}
    </Header>
  );
});
Brand.displayName = "Sidebar.Brand";

export function BrandWordmark({ href, label = "Kaeso" }: { href?: string | undefined; label?: string | undefined }) {
  const LinkComp = useLinkComponent();
  return href ? (
    <LinkComp href={href} aria-label={label} className="-mx-4 flex h-32 items-center rounded-code px-4 text-fg focus-ring">
      <NavWordmark decorative />
    </LinkComp>
  ) : (
    <NavWordmark title={label} className="text-fg" />
  );
}
BrandWordmark.displayName = "Sidebar.BrandWordmark";

const sidebarMenu = lazyModule(() => import("./sidebar-menu.js"), { base: () => import.meta.url });

function LazyMenu({
  trigger,
  side,
  header,
  footer,
  children,
}: {
  trigger: ReactElement;
  side: "top" | "bottom";
  header: ReactNode;
  footer: ReactNode;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const [wanted, setWanted] = useState(false);
  const mod = useLazyModule(sidebarMenu, wanted || open, () => {
    setWanted(false);
    setOpen(false);
  });
  const [store] = useState(triggerPropsStore);
  const button = useRef<HTMLElement | null>(null);
  const pending = open && !mod;
  useEffect(() => whenIdle(() => setWanted(true)), []);
  useEffect(() => {
    if (!pending) return;
    const doc = button.current?.ownerDocument ?? document;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      setOpen(false);
    };
    const onPointerDown = (e: PointerEvent) => {
      if (!button.current?.contains(e.target as Node)) setOpen(false);
    };
    doc.addEventListener("keydown", onKeyDown, true);
    doc.addEventListener("pointerdown", onPointerDown, true);
    return () => {
      doc.removeEventListener("keydown", onKeyDown, true);
      doc.removeEventListener("pointerdown", onPointerDown, true);
    };
  }, [pending]);
  const want = () => setWanted(true);
  const waiting = mod
    ? { ref: button }
    : {
        ref: button,
        "aria-haspopup": "menu",
        "aria-expanded": open,
        ...(open ? { "data-popup-open": "" } : {}),
        onPointerEnter: want,
        onFocus: want,
        onClick: () => setOpen((o) => !o),
        onKeyDown: (e: React.KeyboardEvent) => {
          if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
          e.preventDefault();
          setOpen(true);
        },
        onBlur: (e: React.FocusEvent) => {
          if (open && e.relatedTarget && !e.currentTarget.contains(e.relatedTarget)) setOpen(false);
        },
      };
  return (
    <>
      <HandedTrigger store={store} merge={mod?.mergeTriggerProps} extra={waiting}>
        {trigger}
      </HandedTrigger>
      {mod && (
        <mod.SidebarMenuRoot open={open} onOpenChange={setOpen} triggerProps={store} side={side} header={header} footer={footer}>
          {children}
        </mod.SidebarMenuRoot>
      )}
    </>
  );
}
LazyMenu.displayName = "Sidebar.Menu";

export interface SidebarSwitcherProps {
  avatar: ReactNode;
  label: ReactNode;
  trailing?: ReactNode | undefined;
  caption?: ReactNode | undefined;
  captionTone?: keyof typeof KAESO_SIDEBAR_VARIANTS.caption | undefined;
  children: ReactNode;
  header?: ReactNode | undefined;
  footer?: ReactNode | undefined;
  "aria-label"?: string | undefined;
  className?: string | undefined;
  "data-force"?: string | undefined;
}

function Switcher({
  avatar,
  label,
  trailing,
  caption,
  captionTone = "secondary",
  children,
  header,
  footer,
  "aria-label": ariaLabel,
  className,
  "data-force": force,
}: SidebarSwitcherProps) {
  const { rail } = useSidebar();
  const t = useUiText();
  const captionId = useId();
  const hasCaption = caption !== undefined && caption !== null && caption !== false && caption !== "";
  return (
    <LazyMenu
      side="top"
      header={header}
      footer={footer}
      trigger={
        <button
          type="button"
          aria-label={ariaLabel ?? (typeof label === "string" ? t("sidebar.switchOrOpenSettings", "{label}: switch or open settings", { label }) : undefined)}
          {...(hasCaption ? { "aria-describedby": captionId } : {})}
          {...(force ? { "data-force": force } : {})}
          className={cn(
            "group/sw flex h-36 w-full min-w-0 cursor-pointer items-center gap-8 rounded-pill px-12 text-left text-body font-medium text-fg transition-control hover:bg-fill-2 open:bg-fill-3 focus-ring-inset",
            rail && "justify-center px-0",
            className,
          )}
        >
          {avatar}
          {!rail &&
            (hasCaption ? (
              <span className="flex min-w-0 flex-1 flex-col">
                <span className="truncate">{label}</span>
                <span id={captionId} className={cn("truncate text-caption font-regular", KAESO_SIDEBAR_VARIANTS.caption[captionTone].classes)}>
                  {caption}
                </span>
              </span>
            ) : (
              <span className="min-w-0 flex-1 truncate">{label}</span>
            ))}
          {rail && hasCaption && (
            <span id={captionId} className="sr-only">
              {caption}
            </span>
          )}
          {!rail && trailing}
          {!rail && <Icon icon={icons.switcher} size="sm" className="text-fg-tertiary transition-control group-hover/sw:text-fg-secondary" />}
        </button>
      }
    >
      {children}
    </LazyMenu>
  );
}
Switcher.displayName = "Sidebar.Switcher";

export interface SidebarPlaceProps {
  avatar: ReactNode;
  label: string;
  children?: ReactNode | undefined;
  href?: string | undefined;
  header?: ReactNode | undefined;
  footer?: ReactNode | undefined;
  "aria-label"?: string | undefined;
  className?: string | undefined;
  "data-force"?: string | undefined;
}

function Place({ avatar, label, children, href, header, footer, "aria-label": ariaLabel, className, "data-force": force }: SidebarPlaceProps) {
  const { rail } = useSidebar();
  const t = useUiText();
  const LinkComp = useLinkComponent();
  const classes = cn(
    "group/place flex h-36 w-full min-w-0 items-center gap-8 rounded-pill px-12 text-left text-body font-semibold text-fg",
    (children || href) && "cursor-pointer transition-control hover:bg-fill-2 open:bg-fill-3 focus-ring-inset",
    rail && "justify-center px-0",
    className,
  );
  const inner = (
    <>
      <span className="flex size-18 shrink-0 items-center justify-center">{avatar}</span>
      {rail ? <span className="sr-only">{label}</span> : <PlaceName label={label} />}
      {!rail && children && <Icon icon={icons.switcher} size="sm" className="text-fg-tertiary transition-control group-hover/place:text-fg-secondary" />}
    </>
  );
  const common = { className: classes, ...(force ? { "data-force": force } : {}) };
  if (children) {
    return (
      <LazyMenu
        side="bottom"
        header={header}
        footer={footer}
        trigger={
          <button type="button" aria-label={ariaLabel ?? t("sidebar.switchPlace", "{label}: switch", { label })} {...common}>
            {inner}
          </button>
        }
      >
        {children}
      </LazyMenu>
    );
  }
  const el = href ? (
    <LinkComp href={href} {...(ariaLabel ? { "aria-label": ariaLabel } : {})} {...common}>
      {inner}
    </LinkComp>
  ) : (
    <div {...common}>{inner}</div>
  );
  return rail ? (
    <Tooltip content={label} side="right">
      {el}
    </Tooltip>
  ) : (
    el
  );
}
Place.displayName = "Sidebar.Place";

function PlaceName({ label }: { label: string }) {
  const ref = useRef<HTMLSpanElement>(null);
  const [cut, setCut] = useState(false);
  useLayoutEffect(() => {
    const box = ref.current;
    if (!box || typeof ResizeObserver === "undefined") return;
    const measure = () => setCut(box.scrollWidth > box.clientWidth);
    measure();
    return observeResize([box], measure);
  }, [label]);
  return (
    <Tooltip content={label} side="right" disabled={!cut}>
      <span ref={ref} className="min-w-0 flex-1 truncate">
        {label}
      </span>
    </Tooltip>
  );
}

const Content = forwardRef<HTMLDivElement, HTMLAttributes<HTMLDivElement>>(function SidebarContent({ className, ...props }, ref) {
  const { rail } = useSidebar();
  return (
    <div
      ref={ref}
      className={cn("flex min-h-0 flex-1 flex-col gap-16 overflow-y-auto overscroll-contain pt-8 pb-16 scrollbar-quiet", rail ? "px-8" : "px-16", className)}
      {...props}
    />
  );
});
Content.displayName = "Sidebar.Content";

const Footer = forwardRef<HTMLDivElement, HTMLAttributes<HTMLDivElement>>(function SidebarFooter({ className, ...props }, ref) {
  const { rail } = useSidebar();
  return <div ref={ref} className={cn("flex shrink-0 items-center gap-4 pt-8 pb-12", rail ? "px-8" : "px-16", className)} {...props} />;
});
Footer.displayName = "Sidebar.Footer";

export interface SidebarGroupProps {
  label?: ReactNode | undefined;
  id?: string | undefined;
  defaultOpen?: boolean | undefined;
  collapsible?: boolean | undefined;
  children: ReactNode;
  className?: string | undefined;
}

const Group = forwardRef<HTMLDivElement, SidebarGroupProps>(function SidebarGroup(
  { label, id, defaultOpen = true, collapsible = true, children, className },
  ref,
) {
  const { storageKey, rail } = useSidebar();
  const [open, setOpen] = useStoredState(storageKey && id ? `${storageKey}:group:${id}` : undefined, defaultOpen);
  if (!label || !collapsible || rail) {
    return (
      <div ref={ref} role="group" className={cn("flex flex-col", className)}>
        {label && !rail && <div className="flex h-28 items-center px-12 text-caption font-medium text-fg-secondary">{label}</div>}
        <ul className="flex flex-col gap-2">{children}</ul>
      </div>
    );
  }
  return (
    <Collapsible.Root ref={ref} open={open} onOpenChange={(o: boolean) => setOpen(o)} className={cn("flex flex-col", className)}>
      <Collapsible.Trigger className="group/gl relative -mx-0 flex h-28 w-full cursor-pointer items-center gap-4 rounded-pill px-12 text-left text-caption font-medium text-fg-secondary transition-control hover:text-fg focus-ring tap-target">
        <span>{label}</span>
        <svg viewBox="0 0 8 8" className="size-8 text-fg-tertiary transition-transform duration-180 group-data-panel-open/gl:rotate-90" aria-hidden>
          <path d="M2.5 1.5 5.5 4l-3 2.5z" fill="currentColor" />
        </svg>
      </Collapsible.Trigger>
      <Collapsible.Panel className="collapse-motion">
        <ul className="flex flex-col gap-2 pt-2">{children}</ul>
      </Collapsible.Panel>
    </Collapsible.Root>
  );
});
Group.displayName = "Sidebar.Group";

export interface SidebarItemProps {
  children: ReactNode;
  icon?: IconData | undefined;
  leading?: ReactNode | undefined;
  href?: string | undefined;
  onClick?: ((e: MouseEvent<HTMLElement>) => void) | undefined;
  active?: boolean | undefined;
  count?: number | undefined;
  trailing?: ReactNode | undefined;
  depth?: 0 | 1 | 2 | undefined;
  shortcut?: string[] | string[][] | undefined;
  className?: string | undefined;
  "data-force"?: string | undefined;
}

const DEPTH = ["", "pl-32", "pl-52"] as const;

const Item = forwardRef<HTMLElement, SidebarItemProps>(function SidebarItem(
  { children, icon, leading, href, onClick, active, count, trailing, depth = 0, shortcut, className, "data-force": force },
  ref,
) {
  const { rail } = useSidebar();
  const LinkComp = useLinkComponent();
  const classes = cn(
    "group/si flex h-32 w-full min-w-0 cursor-pointer items-center gap-8 rounded-pill px-12 text-left text-body font-medium transition-control focus-ring-inset",
    active ? KAESO_SIDEBAR_VARIANTS.item.active.classes : KAESO_SIDEBAR_VARIANTS.item.default.classes,
    !rail && DEPTH[depth],
    rail && "justify-center px-0",
    className,
  );
  const inner = (
    <>
      {leading ? (
        <span className="flex size-18 shrink-0 items-center justify-center">{leading}</span>
      ) : (
        icon && <Icon icon={icon} size="md" className={cn("text-fg-secondary transition-control group-hover/si:text-fg", active && "text-fg")} />
      )}
      {!rail && <span className="min-w-0 flex-1 truncate">{children}</span>}
      {!rail && count !== undefined && <span className={cn("text-caption font-medium tabular", active ? "text-fg" : "text-fg-tertiary")}>{count}</span>}
      {!rail && trailing}
    </>
  );
  const common = {
    className: classes,
    ...(active ? { "aria-current": "page" as const } : {}),
    ...(force ? { "data-force": force } : {}),
    ...(rail && typeof children === "string" ? { "aria-label": children } : {}),
  };
  const el = href ? (
    <LinkComp ref={ref as React.Ref<HTMLAnchorElement>} href={href} {...common} {...(onClick ? { onClick } : {})}>
      {inner}
    </LinkComp>
  ) : (
    <button ref={ref as React.Ref<HTMLButtonElement>} type="button" {...common} {...(onClick ? { onClick } : {})}>
      {inner}
    </button>
  );
  return (
    <li className="flex">
      {rail ? (
        <Tooltip content={children} side="right" {...(shortcut ? { shortcut } : {})}>
          {el}
        </Tooltip>
      ) : (
        el
      )}
    </li>
  );
});
Item.displayName = "Sidebar.Item";

const BackLink = forwardRef<HTMLAnchorElement, { href: string; children?: ReactNode | undefined; className?: string | undefined }>(function SidebarBackLink(
  { href, children, className },
  ref,
) {
  const LinkComp = useLinkComponent();
  const t = useUiText();
  return (
    <LinkComp
      ref={ref}
      href={href}
      className={cn(
        "flex h-32 items-center gap-8 rounded-pill px-12 text-body font-medium text-fg-secondary transition-control hover:bg-fill-2 hover:text-fg focus-ring-inset",
        className,
      )}
    >
      <Icon icon={icons.chevronLeft} size="md" />
      {children ?? t("sidebar.backToApp", "Back to app")}
    </LinkComp>
  );
});
BackLink.displayName = "Sidebar.BackLink";

function Loading({ rows = 5 }: { rows?: number | undefined }) {
  return (
    <div className="flex flex-col gap-2" aria-busy>
      {Array.from({ length: rows }, (_, i) => (
        <div key={i} className="flex h-32 items-center gap-8 px-12">
          <Skeleton className="size-18 rounded-pill" />
          <Skeleton className="h-12 rounded-pill" style={{ width: `${48 + ((i * 37) % 40)}%` }} />
        </div>
      ))}
    </div>
  );
}
Loading.displayName = "Sidebar.Loading";

function CollapseToggle({ className }: { className?: string | undefined }) {
  const { collapsed, setCollapsed } = useSidebar();
  const sheet = useContext(SidebarSheetContext);
  const t = useUiText();
  const label = sheet
    ? sheet.peek
      ? t("sidebar.keepOpen", "Keep sidebar open")
      : t("sidebar.closeNavigation", "Close navigation")
    : collapsed
      ? t("sidebar.expand", "Expand sidebar")
      : t("sidebar.collapse", "Collapse sidebar");
  const onClick = () => {
    if (sheet?.peek) setCollapsed(false);
    else if (sheet) sheet.close();
    else setCollapsed(!collapsed);
  };
  return (
    <Tooltip content={label} {...(sheet && !sheet.peek ? {} : { shortcut: ["["] })}>
      <button
        type="button"
        aria-label={label}
        {...(sheet ? {} : { "aria-expanded": !collapsed })}
        onClick={onClick}
        className={cn(
          "inline-flex size-32 cursor-pointer items-center justify-center rounded-pill text-fg-secondary transition-control hover:bg-fill-2 hover:text-fg focus-ring",
          className,
        )}
      >
        <Icon icon={icons.sidebar} size="sm" />
      </button>
    </Tooltip>
  );
}
CollapseToggle.displayName = "Sidebar.CollapseToggle";

function RailToggle({ className }: { className?: string | undefined }) {
  const { rail, setRail } = useSidebar();
  const t = useUiText();
  const label = rail ? t("sidebar.expand", "Expand sidebar") : t("sidebar.collapse", "Collapse sidebar");
  return (
    <Tooltip content={label} side="right">
      <button
        type="button"
        aria-label={label}
        aria-pressed={rail}
        onClick={() => setRail(!rail)}
        className={cn(
          "inline-flex size-32 items-center justify-center rounded-pill text-fg-secondary transition-control hover:bg-fill-2 hover:text-fg focus-ring",
          className,
        )}
      >
        <Icon icon={icons.sidebar} size="sm" />
      </button>
    </Tooltip>
  );
}
RailToggle.displayName = "Sidebar.RailToggle";

export const Sidebar = Object.assign(Root, {
  Provider,
  Header,
  Brand,
  Switcher,
  Place,
  Content,
  Footer,
  Group,
  Item,
  BackLink,
  Loading,
  RailToggle,
  CollapseToggle,
});
