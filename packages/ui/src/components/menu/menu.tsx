import { pageBoundary, usePortalContainer } from "../../utils/portal-context";
import {
  createContext,
  forwardRef,
  useCallback,
  useContext,
  useRef,
  type HTMLAttributes,
  type ReactElement,
  type ReactNode,
  type Ref,
  type RefObject,
} from "react";
import { Menu as BaseMenu } from "@base-ui/react/menu";
import { ContextMenu as BaseContextMenu } from "@base-ui/react/context-menu";
import { Menubar as BaseMenubar } from "@base-ui/react/menubar";
import { cn } from "../../utils/cn";
import { useLinkComponent } from "../../utils/link-provider";
import { Icon, type IconData } from "../icon/icon";
import { KbdSequence } from "../kbd/kbd";
import { icons } from "../../icons/vocabulary";
import { anchorWidthStyle, useLayoutWidth } from "../../hooks/use-layout-width";
import { MENU_DANGER, MENU_HINT, MENU_ICON, MENU_ITEM, MENU_LABEL, MENU_POPUP, MENU_SEPARATOR } from "./styles";

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
        <div tabIndex={0} className="flex min-h-0 flex-1 flex-col overflow-y-auto overscroll-contain rounded-menu p-6 focus-ring-inset scrollbar-quiet">
          <div className="mb-6 px-10 pt-6 pb-10 hairline-b">{header}</div>
          <div role={role} id={id} aria-labelledby={labelledBy} aria-orientation={orientation} className="flex flex-col">
            {children}
          </div>
          {footer && <div className="px-10 pt-6 pb-4 text-caption text-fg-tertiary">{footer}</div>}
        </div>
      </div>
    );
  };
}

const Content = forwardRef<HTMLDivElement, MenuContentProps>(function MenuContent(
  { children, side = "bottom", align = "start", sideOffset = 6, width = "auto", className, header, footer },
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
          {...(header ? { render: framed(header, footer) } : {})}
        >
          {children}
          {!header && footer && <div className="px-10 pt-6 pb-4 text-caption text-fg-tertiary">{footer}</div>}
        </BaseMenu.Popup>
      </BaseMenu.Positioner>
    </BaseMenu.Portal>
  );
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
}: Pick<MenuItemProps, "icon" | "leading" | "description" | "descriptionTone" | "shortcut" | "hint" | "children"> & { danger?: boolean }) {
  return (
    <>
      {leading ? (
        <span className={cn("flex size-16 shrink-0 items-center justify-center", description && "self-start mt-10")}>{leading}</span>
      ) : (
        icon && <Icon icon={icon} size="sm" className={cn(danger ? "text-danger-text" : MENU_ICON, description && "self-start mt-10")} />
      )}
      {description ? (
        <span className="flex min-w-0 flex-1 flex-col gap-2 py-8">
          <span className="truncate">{children}</span>
          <span className={cn("text-small font-regular whitespace-normal", KAESO_MENU_VARIANTS.description[descriptionTone].classes)}>{description}</span>
        </span>
      ) : (
        <span className="min-w-0 flex-1 truncate">{children}</span>
      )}
      {(shortcut || hint) && (
        <span className={MENU_HINT}>
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
  return (
    <BaseMenu.Item
      ref={ref}
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
      >
        {children}
      </ItemContent>
    </BaseMenu.Item>
  );
});
Item.displayName = "Menu.Item";

const LinkItem = forwardRef<HTMLAnchorElement, MenuItemProps & { href: string; external?: boolean | undefined }>(function MenuLinkItem(
  { children, href, external, icon, leading, description, descriptionTone, shortcut, hint, className },
  ref,
) {
  const LinkComp = useLinkComponent();
  return (
    <BaseMenu.LinkItem
      ref={ref}
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
  return (
    <BaseMenu.CheckboxItem
      ref={ref}
      {...(checked !== undefined ? { checked } : {})}
      {...(defaultChecked !== undefined ? { defaultChecked } : {})}
      {...(onCheckedChange ? { onCheckedChange: (c: boolean) => onCheckedChange(c) } : {})}
      {...(disabled ? { disabled } : {})}
      closeOnClick={closeOnClick}
      className={cn(MENU_ITEM, className)}
    >
      <ItemContent icon={icon} leading={leading} description={description} descriptionTone={descriptionTone} shortcut={shortcut} hint={hint}>
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
  return (
    <BaseMenu.RadioItem
      ref={ref}
      value={value}
      {...(disabled ? { disabled } : {})}
      {...(closeOnClick !== undefined ? { closeOnClick } : {})}
      className={cn(MENU_ITEM, className)}
    >
      <ItemContent icon={icon} leading={leading} description={description} descriptionTone={descriptionTone} shortcut={shortcut} hint={hint}>
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

function Separator() {
  return <div role="separator" aria-orientation="horizontal" className={MENU_SEPARATOR} />;
}
Separator.displayName = "Menu.Separator";

function Header({ avatar, name, email }: { avatar?: ReactNode; name: ReactNode; email?: ReactNode }) {
  return (
    <div className="flex min-w-0 items-center gap-10 px-10 pt-8 pb-10">
      {avatar}
      <div className="flex min-w-0 flex-col">
        <span className="truncate text-body font-semibold text-fg">{name}</span>
        {email && <span className="truncate text-caption font-regular text-fg-secondary">{email}</span>}
      </div>
    </div>
  );
}
Header.displayName = "Menu.Header";

function Sub({ children, open, onOpenChange }: { children: ReactNode; open?: boolean; onOpenChange?: (o: boolean) => void }) {
  return (
    <BaseMenu.SubmenuRoot {...(open !== undefined ? { open } : {})} {...(onOpenChange ? { onOpenChange: (o: boolean) => onOpenChange(o) } : {})}>
      {children}
    </BaseMenu.SubmenuRoot>
  );
}
Sub.displayName = "Menu.Sub";

const SubTrigger = forwardRef<HTMLDivElement, Omit<MenuItemProps, "shortcut" | "variant" | "onClick">>(function MenuSubTrigger(
  { children, icon, leading, description, descriptionTone, hint, disabled, className },
  ref,
) {
  return (
    <BaseMenu.SubmenuTrigger ref={ref} {...(disabled ? { disabled } : {})} className={cn(MENU_ITEM, "open:bg-fill-1", className)}>
      <ItemContent icon={icon} leading={leading} description={description} descriptionTone={descriptionTone} hint={hint}>
        {children}
      </ItemContent>
      <Icon icon={icons.chevronRight} size="sm" className="-mr-2 text-fg-tertiary" />
    </BaseMenu.SubmenuTrigger>
  );
});
SubTrigger.displayName = "Menu.SubTrigger";

const SubContent = forwardRef<HTMLDivElement, Omit<MenuContentProps, "side" | "align">>(function MenuSubContent(props, ref) {
  return <Content ref={ref} side="right" align="start" sideOffset={2} {...props} />;
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

function ContextRoot({ children, onOpenChange }: { children: ReactNode; onOpenChange?: (o: boolean) => void }) {
  return <BaseContextMenu.Root {...(onOpenChange ? { onOpenChange: (o: boolean) => onOpenChange(o) } : {})}>{children}</BaseContextMenu.Root>;
}
ContextRoot.displayName = "ContextMenu";
function ContextTrigger({ children, className }: { children: ReactNode; className?: string }) {
  return <BaseContextMenu.Trigger className={className}>{children}</BaseContextMenu.Trigger>;
}
ContextTrigger.displayName = "ContextMenu.Trigger";
const ContextContent = forwardRef<HTMLDivElement, Omit<MenuContentProps, "side" | "align" | "sideOffset">>(function ContextMenuContent(
  { children, className, header, footer, width = "auto" },
  ref,
) {
  return (
    <BaseContextMenu.Portal container={usePortalContainer()}>
      <BaseContextMenu.Positioner positionMethod="fixed" collisionBoundary={pageBoundary()} collisionPadding={8} className="z-50 outline-none">
        <BaseContextMenu.Popup ref={ref} tabIndex={0} className={cn(MENU_POPUP, KAESO_MENU_VARIANTS.width[width].classes, className)}>
          {header && <div className="mb-6 px-10 pt-6 pb-10 hairline-b">{header}</div>}
          {children}
          {footer && <div className="px-10 pt-6 pb-4 text-caption text-fg-tertiary">{footer}</div>}
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

function MenubarRoot({ children, className, "aria-label": ariaLabel }: { children: ReactNode; className?: string; "aria-label"?: string }) {
  return (
    <BaseMenubar className={cn("inline-flex items-center gap-2", className)} {...(ariaLabel ? { "aria-label": ariaLabel } : {})}>
      {children}
    </BaseMenubar>
  );
}
MenubarRoot.displayName = "Menubar";
const MenubarTrigger = forwardRef<HTMLButtonElement, { children: ReactNode; className?: string; "data-force"?: string }>(function MenubarTrigger(
  { children, className, "data-force": force },
  ref,
) {
  return (
    <BaseMenu.Trigger
      ref={ref}
      {...(force ? { "data-force": force } : {})}
      className={cn(
        "inline-flex h-32 cursor-pointer items-center rounded-pill px-12 text-body font-medium text-fg material-ghost transition-control focus-ring",
        className,
      )}
    >
      {children}
    </BaseMenu.Trigger>
  );
});
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
