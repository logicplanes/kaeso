import { pageBoundary, usePortalContainer } from "../../utils/portal-context.js";
import {
  createContext,
  forwardRef,
  useCallback,
  useContext,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type HTMLAttributes,
  type ReactElement,
  type ReactNode,
  type Ref,
  type RefObject,
} from "react";
import { Menu as BaseMenu } from "@base-ui/react/menu";
import { ContextMenu as BaseContextMenu } from "@base-ui/react/context-menu";
import { Menubar as BaseMenubar } from "@base-ui/react/menubar";
import { cn } from "../../utils/cn.js";
import { useLinkComponent } from "../../utils/link-provider.js";
import { Icon, type IconData } from "../icon/icon.js";
import { KbdSequence } from "../kbd/kbd.js";
import { icons } from "../../icons/vocabulary.js";
import { anchorWidthStyle, useLayoutWidth } from "../../hooks/use-layout-width.js";
import { MENU_DANGER, MENU_HINT, MENU_ICON, MENU_ITEM, MENU_LABEL, MENU_POPUP, MENU_SEPARATOR } from "./styles.js";

export const KAESO_MENU_VARIANTS = {
  item: {
    default: { classes: "", description: "An action" },
    danger: { classes: MENU_DANGER, description: "A destructive action: red text, a red tint on highlight" },
  },
  width: {
    auto: { classes: "", description: "220–360px, from its content (default)" },
    trigger: { classes: "w-anchor", description: "As wide as its trigger" },
  },
  description: {
    secondary: { classes: "text-fg-secondary", description: "The second line of a rich item (default)" },
    danger: { classes: "text-danger-text", description: "A second line that warns: an organization scheduled for deletion" },
  },
} as const;
export const KAESO_MENU_DEFAULT_VARIANTS = { item: "default", width: "auto", description: "secondary" } as const;

type Side = "top" | "bottom" | "left" | "right";
type Align = "start" | "center" | "end";

export interface MenuProps {
  children: ReactNode;
  open?: boolean | undefined;
  defaultOpen?: boolean | undefined;
  onOpenChange?: ((open: boolean) => void) | undefined;
  modal?: boolean | undefined;
}

const NO_TRIGGER: RefObject<HTMLElement | null> = { current: null };
const TriggerContext = createContext<RefObject<HTMLElement | null> | null>(null);

function Root({ open, defaultOpen, onOpenChange, modal = false, children }: MenuProps) {
  const trigger = useRef<HTMLElement | null>(null);
  return (
    <BaseMenu.Root
      {...(open !== undefined ? { open } : {})}
      {...(defaultOpen !== undefined ? { defaultOpen } : {})}
      {...(onOpenChange ? { onOpenChange: (o: boolean) => onOpenChange(o) } : {})}
      modal={modal}
    >
      <TriggerContext.Provider value={trigger}>{children}</TriggerContext.Provider>
    </BaseMenu.Root>
  );
}
Root.displayName = "Menu";

function Trigger({ children, openOnHover }: { children: ReactElement; openOnHover?: boolean | undefined }) {
  const trigger = useContext(TriggerContext);
  const setTrigger = useCallback(
    (el: HTMLElement | null) => {
      if (trigger) trigger.current = el;
    },
    [trigger],
  );
  return <BaseMenu.Trigger ref={setTrigger} render={children} {...(openOnHover ? { openOnHover } : {})} />;
}
Trigger.displayName = "Menu.Trigger";

export interface MenuContentProps {
  children: ReactNode;
  side?: Side | undefined;
  align?: Align | undefined;
  sideOffset?: number | undefined;
  width?: keyof typeof KAESO_MENU_VARIANTS.width | undefined;
  className?: string | undefined;
  header?: ReactNode | undefined;
  footer?: ReactNode | undefined;
}

function framed(header: ReactNode, footer: ReactNode) {
  return function framedMenuPopup(props: HTMLAttributes<HTMLDivElement> & { ref?: Ref<HTMLDivElement> | undefined }) {
    const { role, id, "aria-labelledby": labelledBy, "aria-orientation": orientation, children, className, ...rest } = props;
    return (
      <div {...rest} className={cn(className, "overflow-hidden p-0")}>
        <div className="flex min-h-0 flex-1 flex-col overflow-y-auto overscroll-contain rounded-menu p-6 scrollbar-quiet">
          {header && <div className="mb-6 px-10 pt-6 pb-10 hairline-b">{header}</div>}
          <div role={role} id={id} aria-labelledby={labelledBy} aria-orientation={orientation} className="flex flex-col">
            {children}
          </div>
          {footer && <div className="px-10 pt-6 pb-4 text-caption text-fg-tertiary">{footer}</div>}
        </div>
      </div>
    );
  };
}

const Popup = forwardRef<HTMLDivElement, MenuContentProps & { alignOffset?: number | undefined }>(function MenuPopup(
  { children, side = "bottom", align = "start", sideOffset = 6, alignOffset = 0, width = "auto", className, header, footer },
  ref,
) {
  const trigger = useContext(TriggerContext);
  const triggerWidth = useLayoutWidth(trigger ?? NO_TRIGGER, width === "trigger" && !!trigger);
  const popup = useRef<HTMLDivElement | null>(null);
  return (
    <BaseMenu.Portal container={usePortalContainer()}>
      <BaseMenu.Positioner
        positionMethod="fixed"
        collisionBoundary={pageBoundary()}
        side={side}
        align={align}
        sideOffset={sideOffset}
        alignOffset={alignOffset}
        collisionPadding={8}
        className="z-50 outline-none"
      >
        <BaseMenu.Popup
          ref={(el) => {
            popup.current = el;
            if (typeof ref === "function") ref(el);
            else if (ref) ref.current = el;
          }}
          finalFocus={() => {
            const at = document.activeElement;
            if (at && at !== document.body && !popup.current?.contains(at)) return false;
            return trigger?.current?.isConnected ? trigger.current : true;
          }}
          tabIndex={0}
          className={cn(MENU_POPUP, KAESO_MENU_VARIANTS.width[width].classes, className)}
          style={width === "trigger" ? anchorWidthStyle(triggerWidth) : undefined}
          {...(header || footer ? { render: framed(header, footer) } : {})}
        >
          {children}
        </BaseMenu.Popup>
      </BaseMenu.Positioner>
    </BaseMenu.Portal>
  );
});
Popup.displayName = "Menu.Popup";

const Content = forwardRef<HTMLDivElement, MenuContentProps>(function MenuContent(props, ref) {
  return <Popup ref={ref} {...props} />;
});
Content.displayName = "Menu.Content";

export interface MenuItemProps {
  children: ReactNode;
  icon?: IconData | undefined;
  leading?: ReactNode | undefined;
  description?: ReactNode | undefined;
  descriptionTone?: keyof typeof KAESO_MENU_VARIANTS.description | undefined;
  shortcut?: string[] | string[][] | undefined;
  hint?: ReactNode | undefined;
  variant?: keyof typeof KAESO_MENU_VARIANTS.item | undefined;
  disabled?: boolean | undefined;
  onClick?: (() => void) | undefined;
  closeOnClick?: boolean | undefined;
  className?: string | undefined;
  "data-force"?: string | undefined;
}

function ItemContent({
  icon,
  leading,
  description,
  descriptionTone = "secondary",
  shortcut,
  hint,
  children,
  danger,
  hintId,
}: Pick<MenuItemProps, "icon" | "leading" | "description" | "descriptionTone" | "shortcut" | "hint" | "children"> & {
  danger?: boolean | undefined;
  hintId?: string | undefined;
}) {
  return (
    <>
      {leading ? (
        <span className={cn("flex size-16 shrink-0 items-center justify-center", description && "self-start mt-10")}>{leading}</span>
      ) : (
        icon && <Icon icon={icon} size="sm" className={cn(danger ? "text-danger-text" : MENU_ICON, description && "self-start mt-10")} />
      )}
      {description ? (
        <span className="flex min-w-0 flex-1 flex-col gap-2 py-8">
          <span className="text-pretty wrap-break-word">{children}</span>
          <span className={cn("text-small font-regular whitespace-normal", KAESO_MENU_VARIANTS.description[descriptionTone].classes)}>{description}</span>
        </span>
      ) : (
        <span className="min-w-0 flex-1 py-6 text-pretty wrap-break-word">{children}</span>
      )}
      {(shortcut || hint) && (
        <span id={hintId} aria-hidden className={MENU_HINT}>
          {hint}
          {shortcut && <KbdSequence keys={shortcut} />}
        </span>
      )}
    </>
  );
}

const Item = forwardRef<HTMLDivElement, MenuItemProps>(function MenuItem(
  {
    children,
    icon,
    leading,
    description,
    descriptionTone,
    shortcut,
    hint,
    variant = "default",
    disabled,
    onClick,
    closeOnClick,
    className,
    "data-force": force,
  },
  ref,
) {
  const hintId = useId();
  return (
    <BaseMenu.Item
      ref={ref}
      {...(shortcut || hint ? { "aria-describedby": hintId } : {})}
      {...(disabled ? { disabled } : {})}
      {...(onClick ? { onClick } : {})}
      {...(closeOnClick !== undefined ? { closeOnClick } : {})}
      {...(force ? { "data-force": force } : {})}
      className={cn(MENU_ITEM, KAESO_MENU_VARIANTS.item[variant].classes, className)}
    >
      <ItemContent
        icon={icon}
        leading={leading}
        description={description}
        descriptionTone={descriptionTone}
        shortcut={shortcut}
        hint={hint}
        danger={variant === "danger"}
        hintId={hintId}
      >
        {children}
      </ItemContent>
    </BaseMenu.Item>
  );
});
Item.displayName = "Menu.Item";

export interface MenuLinkItemProps extends Pick<
  MenuItemProps,
  "icon" | "leading" | "description" | "descriptionTone" | "shortcut" | "hint" | "children" | "className"
> {
  href: string;
  external?: boolean | undefined;
}

const LinkItem = forwardRef<HTMLAnchorElement, MenuLinkItemProps>(function MenuLinkItem(
  { children, href, external, icon, leading, description, descriptionTone, shortcut, hint, className },
  ref,
) {
  const LinkComp = useLinkComponent();
  const hintId = useId();
  return (
    <BaseMenu.LinkItem
      ref={ref}
      {...(shortcut || hint ? { "aria-describedby": hintId } : {})}
      href={href}
      {...(external ? { target: "_blank", rel: "noopener noreferrer" } : { render: <LinkComp href={href} /> })}
      className={cn(MENU_ITEM, "no-underline", className)}
    >
      <ItemContent
        icon={icon}
        leading={leading}
        description={description}
        descriptionTone={descriptionTone}
        shortcut={shortcut}
        hint={hint ?? (external ? <Icon icon={icons.externalLink} size="xs" /> : undefined)}
        hintId={hintId}
      >
        {children}
      </ItemContent>
    </BaseMenu.LinkItem>
  );
});
LinkItem.displayName = "Menu.LinkItem";

export interface MenuCheckboxItemProps extends Omit<MenuItemProps, "variant" | "onClick"> {
  checked?: boolean | undefined;
  defaultChecked?: boolean | undefined;
  onCheckedChange?: ((checked: boolean) => void) | undefined;
}

const CheckboxItem = forwardRef<HTMLDivElement, MenuCheckboxItemProps>(function MenuCheckboxItem(
  {
    children,
    icon,
    leading,
    description,
    descriptionTone,
    shortcut,
    hint,
    checked,
    defaultChecked,
    onCheckedChange,
    disabled,
    closeOnClick = false,
    className,
  },
  ref,
) {
  const hintId = useId();
  return (
    <BaseMenu.CheckboxItem
      ref={ref}
      {...(shortcut || hint ? { "aria-describedby": hintId } : {})}
      {...(checked !== undefined ? { checked } : {})}
      {...(defaultChecked !== undefined ? { defaultChecked } : {})}
      {...(onCheckedChange ? { onCheckedChange: (c: boolean) => onCheckedChange(c) } : {})}
      {...(disabled ? { disabled } : {})}
      closeOnClick={closeOnClick}
      className={cn(MENU_ITEM, className)}
    >
      <ItemContent icon={icon} leading={leading} description={description} descriptionTone={descriptionTone} shortcut={shortcut} hint={hint} hintId={hintId}>
        {children}
      </ItemContent>
      <BaseMenu.CheckboxItemIndicator keepMounted className="flex size-16 shrink-0 items-center justify-center text-fg opacity-0 data-checked:opacity-100">
        <Icon icon={icons.check} size="sm" />
      </BaseMenu.CheckboxItemIndicator>
    </BaseMenu.CheckboxItem>
  );
});
CheckboxItem.displayName = "Menu.CheckboxItem";

const RadioGroup = forwardRef<
  HTMLDivElement,
  { value?: string | undefined; defaultValue?: string | undefined; onValueChange?: ((v: string) => void) | undefined; children: ReactNode }
>(function MenuRadioGroup({ value, defaultValue, onValueChange, children }, ref) {
  return (
    <BaseMenu.RadioGroup
      ref={ref}
      {...(value !== undefined ? { value } : {})}
      {...(defaultValue !== undefined ? { defaultValue } : {})}
      {...(onValueChange ? { onValueChange: (v: unknown) => onValueChange(String(v)) } : {})}
    >
      {children}
    </BaseMenu.RadioGroup>
  );
});
RadioGroup.displayName = "Menu.RadioGroup";

const RadioItem = forwardRef<HTMLDivElement, Omit<MenuItemProps, "variant"> & { value: string }>(function MenuRadioItem(
  { children, value, icon, leading, description, descriptionTone, shortcut, hint, disabled, closeOnClick, className },
  ref,
) {
  const hintId = useId();
  return (
    <BaseMenu.RadioItem
      ref={ref}
      {...(shortcut || hint ? { "aria-describedby": hintId } : {})}
      value={value}
      {...(disabled ? { disabled } : {})}
      {...(closeOnClick !== undefined ? { closeOnClick } : {})}
      className={cn(MENU_ITEM, className)}
    >
      <ItemContent icon={icon} leading={leading} description={description} descriptionTone={descriptionTone} shortcut={shortcut} hint={hint} hintId={hintId}>
        {children}
      </ItemContent>
      <BaseMenu.RadioItemIndicator keepMounted className="flex size-16 shrink-0 items-center justify-center text-fg opacity-0 data-checked:opacity-100">
        <Icon icon={icons.check} size="sm" />
      </BaseMenu.RadioItemIndicator>
    </BaseMenu.RadioItem>
  );
});
RadioItem.displayName = "Menu.RadioItem";

const Group = forwardRef<HTMLDivElement, { label?: ReactNode | undefined; children: ReactNode }>(function MenuGroup({ label, children }, ref) {
  return (
    <BaseMenu.Group ref={ref} className="flex flex-col">
      {label && <BaseMenu.GroupLabel className={MENU_LABEL}>{label}</BaseMenu.GroupLabel>}
      {children}
    </BaseMenu.Group>
  );
});
Group.displayName = "Menu.Group";

const Separator = forwardRef<HTMLDivElement>(function MenuSeparator(_props, ref) {
  return <div ref={ref} role="separator" aria-orientation="horizontal" className={MENU_SEPARATOR} />;
});
Separator.displayName = "Menu.Separator";

const Header = forwardRef<HTMLDivElement, { avatar?: ReactNode | undefined; name: ReactNode; email?: ReactNode | undefined }>(function MenuHeader(
  { avatar, name, email },
  ref,
) {
  return (
    <div ref={ref} className="flex min-w-0 items-center gap-10 px-10 pt-8 pb-10">
      {avatar}
      <div className="flex min-w-0 flex-col">
        <span className="text-body font-semibold text-fg wrap-break-word">{name}</span>
        {email && <span className="text-caption font-regular text-fg-secondary wrap-anywhere">{email}</span>}
      </div>
    </div>
  );
});
Header.displayName = "Menu.Header";

function Sub({ children, open, onOpenChange }: { children: ReactNode; open?: boolean | undefined; onOpenChange?: ((o: boolean) => void) | undefined }) {
  const trigger = useRef<HTMLElement | null>(null);
  return (
    <BaseMenu.SubmenuRoot {...(open !== undefined ? { open } : {})} {...(onOpenChange ? { onOpenChange: (o: boolean) => onOpenChange(o) } : {})}>
      <TriggerContext.Provider value={trigger}>{children}</TriggerContext.Provider>
    </BaseMenu.SubmenuRoot>
  );
}
Sub.displayName = "Menu.Sub";

const SubTrigger = forwardRef<HTMLDivElement, Omit<MenuItemProps, "shortcut" | "variant" | "onClick">>(function MenuSubTrigger(
  { children, icon, leading, description, descriptionTone, hint, disabled, className },
  ref,
) {
  const trigger = useContext(TriggerContext);
  const setRef = useCallback(
    (el: HTMLDivElement | null) => {
      if (trigger) trigger.current = el;
      if (typeof ref === "function") ref(el);
      else if (ref) ref.current = el;
    },
    [trigger, ref],
  );
  const hintId = useId();
  return (
    <BaseMenu.SubmenuTrigger
      ref={setRef}
      {...(disabled ? { disabled } : {})}
      {...(hint ? { "aria-describedby": hintId } : {})}
      className={cn(MENU_ITEM, "open:bg-item-hover", className)}
    >
      <ItemContent icon={icon} leading={leading} description={description} descriptionTone={descriptionTone} hint={hint} hintId={hintId}>
        {children}
      </ItemContent>
      <Icon icon={icons.chevronRight} size="sm" className="-mr-2 text-fg-tertiary" />
    </BaseMenu.SubmenuTrigger>
  );
});
SubTrigger.displayName = "Menu.SubTrigger";

const SubContent = forwardRef<HTMLDivElement, Omit<MenuContentProps, "side" | "align">>(function MenuSubContent(props, ref) {
  return <Popup ref={ref} side="right" align="start" sideOffset={8} alignOffset={-6} {...props} />;
});
SubContent.displayName = "Menu.SubContent";

export const Menu = Object.assign(Root, {
  Trigger,
  Content,
  Item,
  LinkItem,
  CheckboxItem,
  RadioGroup,
  RadioItem,
  Group,
  Separator,
  Header,
  Sub,
  SubTrigger,
  SubContent,
});

export const KAESO_CONTEXT_MENU_VARIANTS = {} as const;
export const KAESO_CONTEXT_MENU_DEFAULT_VARIANTS = {} as const;

const FOCUSABLE = 'a[href], button, input, select, textarea, [tabindex]:not([tabindex="-1"])';

function ContextRoot({ children, onOpenChange }: { children: ReactNode; onOpenChange?: ((open: boolean) => void) | undefined }) {
  const opener = useRef<HTMLElement | null>(null);
  return (
    <BaseContextMenu.Root
      onOpenChange={(o: boolean) => {
        if (o) {
          const at = document.activeElement;
          opener.current = at instanceof HTMLElement && at !== document.body ? at : null;
        }
        onOpenChange?.(o);
      }}
    >
      <TriggerContext.Provider value={opener}>{children}</TriggerContext.Provider>
    </BaseContextMenu.Root>
  );
}
ContextRoot.displayName = "ContextMenu";
const ContextTrigger = forwardRef<HTMLDivElement, { children: ReactNode; className?: string | undefined; "data-force"?: string | undefined }>(
  function ContextMenuTrigger({ children, className, "data-force": force }, ref) {
    const own = useRef<HTMLDivElement | null>(null);
    const [focusable, setFocusable] = useState(false);
    useLayoutEffect(() => {
      setFocusable(!own.current?.querySelector(FOCUSABLE));
    }, [children]);
    return (
      <BaseContextMenu.Trigger
        ref={(el: HTMLDivElement | null) => {
          own.current = el;
          if (typeof ref === "function") ref(el);
          else if (ref) ref.current = el;
        }}
        {...(focusable ? { tabIndex: 0 } : {})}
        {...(force ? { "data-force": force } : {})}
        className={cn(focusable && "focus-ring", className)}
      >
        {children}
      </BaseContextMenu.Trigger>
    );
  },
);
ContextTrigger.displayName = "ContextMenu.Trigger";
const ContextContent = forwardRef<HTMLDivElement, Omit<MenuContentProps, "side" | "align" | "sideOffset">>(function ContextMenuContent(
  { children, className, header, footer, width = "auto" },
  ref,
) {
  const opener = useContext(TriggerContext);
  const popup = useRef<HTMLDivElement | null>(null);
  return (
    <BaseContextMenu.Portal container={usePortalContainer()}>
      <BaseContextMenu.Positioner positionMethod="fixed" collisionBoundary={pageBoundary()} collisionPadding={8} className="z-50 outline-none">
        <BaseContextMenu.Popup
          ref={(el: HTMLDivElement | null) => {
            popup.current = el;
            if (typeof ref === "function") ref(el);
            else if (ref) ref.current = el;
          }}
          finalFocus={() => {
            const at = document.activeElement;
            if (at && at !== document.body && !popup.current?.contains(at)) return false;
            return opener?.current?.isConnected ? opener.current : true;
          }}
          tabIndex={0}
          className={cn(MENU_POPUP, KAESO_MENU_VARIANTS.width[width].classes, className)}
          {...(header || footer ? { render: framed(header, footer) } : {})}
        >
          {children}
        </BaseContextMenu.Popup>
      </BaseContextMenu.Positioner>
    </BaseContextMenu.Portal>
  );
});
ContextContent.displayName = "ContextMenu.Content";
export const ContextMenu = Object.assign(ContextRoot, {
  Trigger: ContextTrigger,
  Content: ContextContent,
  Item,
  LinkItem,
  CheckboxItem,
  RadioGroup,
  RadioItem,
  Group,
  Separator,
  Sub,
  SubTrigger,
  SubContent,
});

export const KAESO_MENUBAR_VARIANTS = {} as const;
export const KAESO_MENUBAR_DEFAULT_VARIANTS = {} as const;

const MenubarRoot = forwardRef<HTMLDivElement, { children: ReactNode; className?: string | undefined; "aria-label"?: string | undefined }>(function Menubar(
  { children, className, "aria-label": ariaLabel },
  ref,
) {
  return (
    <BaseMenubar ref={ref} className={cn("inline-flex items-center gap-8", className)} {...(ariaLabel ? { "aria-label": ariaLabel } : {})}>
      {children}
    </BaseMenubar>
  );
});
MenubarRoot.displayName = "Menubar";
const MenubarTrigger = forwardRef<HTMLButtonElement, { children: ReactNode; className?: string | undefined; "data-force"?: string | undefined }>(
  function MenubarTrigger({ children, className, "data-force": force }, ref) {
    return (
      <BaseMenu.Trigger
        ref={ref}
        {...(force ? { "data-force": force } : {})}
        className={cn(
          "relative inline-flex h-32 cursor-pointer items-center rounded-pill px-12 text-body font-medium text-fg material-ghost transition-control focus-ring tap-target",
          className,
        )}
      >
        {children}
      </BaseMenu.Trigger>
    );
  },
);
MenubarTrigger.displayName = "Menubar.Trigger";
export const Menubar = Object.assign(MenubarRoot, {
  Menu: Root,
  Trigger: MenubarTrigger,
  Content,
  Item,
  CheckboxItem,
  RadioGroup,
  RadioItem,
  Group,
  Separator,
  Sub,
  SubTrigger,
  SubContent,
});
