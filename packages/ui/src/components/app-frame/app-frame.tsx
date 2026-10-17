import { usePortalContainer } from "../../utils/portal-context.js";
import { createContext, forwardRef, useContext, useId, useState, type HTMLAttributes, type ReactNode, type RefObject } from "react";
import { Dialog } from "@base-ui/react/dialog";
import { InsetContent } from "../../utils/inset.js";
import { cn } from "../../utils/cn.js";
import { icons } from "../../icons/vocabulary.js";
import { IconButton } from "../icon-button/icon-button.js";
import { buttonClasses } from "../button/button.js";
import { useSidebar, BrandWordmark, SidebarFloatingContext, SidebarSheetContext } from "../sidebar/sidebar.js";
import { useEffect, useRef } from "react";
import { useUiText } from "../../i18n/context.js";

export const KAESO_APP_FRAME_VARIANTS = {
  fill: {
    screen: { classes: "h-dvh", description: "Fills the viewport (default)" },
    parent: { classes: "h-full", description: "Fills its container: previews, embedded tools" },
  },
} as const;
export const KAESO_APP_FRAME_DEFAULT_VARIANTS = { fill: "screen" } as const;

const Ctx = createContext<{ openNav: () => void; embedded: boolean }>({ openNav: () => undefined, embedded: false });
export const useAppFrame = () => useContext(Ctx);

export interface AppFrameProps {
  sidebar: ReactNode;
  children: ReactNode;
  corner?: ReactNode | undefined;
  fill?: keyof typeof KAESO_APP_FRAME_VARIANTS.fill | undefined;
  brandHref?: string | undefined;
  brandLabel?: string | undefined;
  phoneActions?: ReactNode | undefined;
  className?: string | undefined;
}

const Root = forwardRef<HTMLDivElement, AppFrameProps>(function AppFrame(
  { sidebar, children, corner, fill = "screen", brandHref, brandLabel, phoneActions, className },
  ref,
) {
  const [navOpen, setNavOpen] = useState(false);
  const toggle = useRef<HTMLButtonElement | null>(null);
  const mainId = `k-main-${useId().replace(/[^\w-]/g, "")}`;
  const Main = fill === "screen" ? "main" : "div";
  return (
    <Ctx.Provider value={{ openNav: () => setNavOpen(true), embedded: fill === "parent" }}>
      <div
        ref={ref}
        className={cn(
          "relative isolate flex w-full flex-col overflow-hidden bg-chrome text-fg lg:flex-row",
          KAESO_APP_FRAME_VARIANTS.fill[fill].classes,
          className,
        )}
      >
        <SkipLink target={mainId} />
        <div className="hidden h-full lg:flex">{sidebar}</div>
        <PhoneBar
          banner={fill === "screen"}
          href={brandHref}
          label={brandLabel}
          actions={phoneActions}
          open={navOpen}
          onOpen={() => setNavOpen(true)}
          toggleRef={toggle}
        />
        <Main
          id={mainId}
          className="relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-panel outline-none safe-area-panel lg:my-8 lg:mr-8 lg:rounded-panel lg:shadow-panel"
        >
          {children}
        </Main>
        {corner && <div className="fixed right-16 bottom-16 z-10 flex items-center gap-8">{corner}</div>}
      </div>
      <FrameSheet navOpen={navOpen} setNavOpen={setNavOpen} returnFocus={toggle}>
        {sidebar}
      </FrameSheet>
    </Ctx.Provider>
  );
});
Root.displayName = "AppFrame";

function SkipLink({ target }: { target: string }) {
  const t = useUiText();
  return (
    <a
      href={`#${target}`}
      onClick={(e) => {
        const main = document.getElementById(target);
        if (!main) return;
        e.preventDefault();
        main.setAttribute("tabindex", "-1");
        main.addEventListener("blur", () => main.removeAttribute("tabindex"), { once: true });
        main.focus();
      }}
      className={cn(buttonClasses({ size: "sm" }), "absolute top-8 left-8 z-50 not-focus:sr-only")}
    >
      {t("appFrame.skipToContent", "Skip to content")}
    </a>
  );
}
SkipLink.displayName = "AppFrame.SkipLink";

function PhoneBar({
  banner,
  href,
  label,
  actions,
  open,
  onOpen,
  toggleRef,
}: {
  banner: boolean;
  href: string | undefined;
  label: string | undefined;
  actions: ReactNode;
  open: boolean;
  onOpen: () => void;
  toggleRef: RefObject<HTMLButtonElement | null>;
}) {
  const Bar = banner ? "header" : "div";
  const t = useUiText();
  return (
    <Bar data-phone-bar="" className="shrink-0 bg-panel safe-area-bar lg:hidden">
      <div className="mt-8 flex h-52 items-center justify-between pr-20 pl-28">
        <BrandWordmark href={href} label={label} />
        <span className="flex items-center">
          {actions}
          <IconButton
            ref={toggleRef}
            icon={icons.sidebar}
            aria-label={t("appFrame.openNavigation", "Open navigation")}
            aria-haspopup="dialog"
            aria-expanded={open}
            tooltip={false}
            onClick={onOpen}
          />
        </span>
      </div>
    </Bar>
  );
}
PhoneBar.displayName = "AppFrame.PhoneBar";

export function NavSheet({
  open,
  onOpenChange,
  peek = false,
  returnFocus,
  children,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  peek?: boolean | undefined;
  returnFocus?: RefObject<HTMLElement | null> | undefined;
  children: ReactNode;
}) {
  const leave = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const t = useUiText();
  useEffect(() => {
    if (!open || peek || typeof window === "undefined") return;
    const wide = window.matchMedia("(min-width: 1024px)");
    const onChange = () => wide.matches && onOpenChange(false);
    onChange();
    wide.addEventListener("change", onChange);
    return () => wide.removeEventListener("change", onChange);
  }, [open, peek, onOpenChange]);
  return (
    <Dialog.Root open={open} onOpenChange={(o: boolean) => onOpenChange(o)} modal={!peek}>
      <Dialog.Portal container={usePortalContainer()}>
        <Dialog.Backdrop className="fixed inset-0 z-40 bg-overlay opacity-60 backdrop-motion" />
        <Dialog.Popup
          aria-label={t("appFrame.navigation", "Navigation")}
          data-side="left"
          initialFocus={!peek}
          {...(returnFocus ? { finalFocus: returnFocus } : {})}
          className="fixed top-0 bottom-0 left-0 z-40 flex overflow-hidden rounded-r-sheet bg-chrome shadow-dialog outline-none safe-area-sheet sheet-motion"
          onClick={(e) => {
            if ((e.target as HTMLElement).closest("a[href]")) onOpenChange(false);
          }}
          onPointerEnter={() => clearTimeout(leave.current)}
          onPointerLeave={() => {
            if (!peek) return;
            leave.current = setTimeout(() => onOpenChange(false), 160);
          }}
        >
          <SidebarFloatingContext.Provider value>
            <SidebarSheetContext.Provider value={{ close: () => onOpenChange(false), peek }}>{children}</SidebarSheetContext.Provider>
          </SidebarFloatingContext.Provider>
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
NavSheet.displayName = "NavSheet";

export const PEEK_DELAY = 750;

function FrameSheet({
  navOpen,
  setNavOpen,
  returnFocus,
  children,
}: {
  navOpen: boolean;
  setNavOpen: (o: boolean) => void;
  returnFocus: RefObject<HTMLElement | null>;
  children: ReactNode;
}) {
  const { peeking, setPeeking } = useSidebar();
  return (
    <NavSheet
      open={navOpen || peeking}
      peek={peeking && !navOpen}
      {...(navOpen ? { returnFocus } : {})}
      onOpenChange={(o) => {
        if (!o) {
          setNavOpen(false);
          setPeeking(false);
        }
      }}
    >
      {children}
    </NavSheet>
  );
}

function MenuButton({ className }: { className?: string | undefined }) {
  const { openNav } = useAppFrame();
  const { collapsed, setCollapsed, setPeeking } = useSidebar();
  const intent = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const t = useUiText();
  const wide = () => typeof window !== "undefined" && window.matchMedia("(min-width: 1024px)").matches;
  return (
    <IconButton
      onPointerEnter={() => {
        if (collapsed && wide()) intent.current = setTimeout(() => setPeeking(true), PEEK_DELAY);
      }}
      onPointerLeave={() => clearTimeout(intent.current)}
      icon={icons.sidebar}
      data-sidebar-expand=""
      aria-label={collapsed ? t("appFrame.expandSidebar", "Expand sidebar") : t("appFrame.openNavigation", "Open navigation")}
      shortcut={collapsed ? ["["] : undefined}
      onClick={() => {
        clearTimeout(intent.current);
        if (collapsed && wide()) setCollapsed(false);
        else openNav();
      }}
      className={cn("hidden", collapsed && "lg:inline-flex", className)}
    />
  );
}
MenuButton.displayName = "AppFrame.MenuButton";

export const KAESO_APP_FRAME_BODY_VARIANTS = {
  padding: {
    none: { classes: "", description: "Edge to edge: lists and tables that bring their own row insets (default)" },
    list: { classes: "pt-8 pb-24", description: "A list or table under the header: 8px above the first row; rows bring their own side insets" },
    page: { classes: "px-24 pt-16 pb-48", description: "A content page (overviews, charts, sections): 24px sides, 16px under the header, 48px at the end" },
  },
} as const;
export const KAESO_APP_FRAME_BODY_DEFAULT_VARIANTS = { padding: "none" } as const;

export interface AppFrameBodyProps extends HTMLAttributes<HTMLDivElement> {
  padding?: keyof typeof KAESO_APP_FRAME_BODY_VARIANTS.padding | undefined;
}

const Body = forwardRef<HTMLDivElement, AppFrameBodyProps>(function AppFrameBody({ className, padding = "none", ...props }, ref) {
  return (
    <div
      ref={ref}
      className={cn("min-h-0 flex-1 overflow-y-auto overscroll-contain scrollbar-quiet", KAESO_APP_FRAME_BODY_VARIANTS.padding[padding].classes, className)}
      {...props}
    />
  );
});
Body.displayName = "AppFrame.Body";

export interface SettingsPageProps extends Omit<HTMLAttributes<HTMLDivElement>, "title"> {
  title: ReactNode;
  description?: ReactNode | undefined;
  actions?: ReactNode | undefined;
}

const Settings = forwardRef<HTMLDivElement, SettingsPageProps>(function AppFrameSettings({ title, description, actions, className, children, ...props }, ref) {
  return (
    <Body ref={ref} className="relative">
      <MenuButton className="absolute top-10 left-12 z-10" />
      <div className={cn("mx-auto w-full max-w-settings px-24 pt-64 pb-96", className)} {...props}>
        <header className="flex flex-col gap-8">
          <div className="flex items-start justify-between gap-16">
            <h1 className="min-w-0 text-title-lg font-semibold text-fg text-balance">{title}</h1>
            {actions && <div className="flex h-34 shrink-0 items-center gap-8">{actions}</div>}
          </div>
          {description && <p className="text-body-lg font-regular text-fg-secondary">{description}</p>}
        </header>
        <div className="mt-8">
          <InsetContent>{children}</InsetContent>
        </div>
      </div>
    </Body>
  );
});
Settings.displayName = "AppFrame.Settings";

export const AppFrame = Object.assign(Root, { MenuButton, Body, Settings });
