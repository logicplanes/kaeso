import { createContext, forwardRef, useContext, type ButtonHTMLAttributes, type HTMLAttributes, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { Icon, type IconData } from "../icon/icon.js";
import { BUTTON_ICON_SIZE, KAESO_BUTTON_VARIANTS, type ButtonSize } from "../button/button.js";
import { Tooltip } from "../tooltip/tooltip.js";
import { warnMissingName } from "../../utils/dev.js";

export const KAESO_BUTTON_GROUP_VARIANTS = {
  attached: {
    true: { classes: "inline-flex rounded-pill bg-raised ring-over isolate", description: "Connected: one outer pill with inner hairline dividers (default)" },
    false: { classes: "inline-flex gap-8", description: "Spaced: separate pills 8px apart" },
  },
  size: {
    xs: { classes: "", description: "24px" },
    sm: { classes: "", description: "32px" },
    md: { classes: "", description: "36px (default)" },
    lg: { classes: "", description: "40px" },
  },
} as const;
export const KAESO_BUTTON_GROUP_DEFAULT_VARIANTS = { attached: "true", size: "md" } as const;

type GroupSize = Exclude<ButtonSize, "xl">;
const ICON_ONLY: Record<GroupSize, string> = { xs: "w-28 px-0", sm: "w-36 px-0", md: "w-40 px-0", lg: "w-44 px-0" };

const Ctx = createContext<{ size: GroupSize; attached: boolean }>({ size: "md", attached: true });

export interface ButtonGroupProps extends HTMLAttributes<HTMLDivElement> {
  size?: GroupSize | undefined;
  attached?: boolean | undefined;
  "aria-label"?: string | undefined;
  children: ReactNode;
}

const Root = forwardRef<HTMLDivElement, ButtonGroupProps>(function ButtonGroup({ size = "md", attached = true, className, children, ...props }, ref) {
  return (
    <Ctx.Provider value={{ size, attached }}>
      <div ref={ref} role="group" className={cn(KAESO_BUTTON_GROUP_VARIANTS.attached[attached ? "true" : "false"].classes, className)} {...props}>
        {children}
      </div>
    </Ctx.Provider>
  );
});
Root.displayName = "ButtonGroup";

export interface ButtonGroupItemProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children"> {
  children?: ReactNode | undefined;
  icon?: IconData | undefined;
  "aria-label"?: string | undefined;
  pressed?: boolean | undefined;
  "data-force"?: string | undefined;
}

const Item = forwardRef<HTMLButtonElement, ButtonGroupItemProps>(function ButtonGroupItem({ icon, children, pressed, className, ...props }, ref) {
  const { size, attached } = useContext(Ctx);
  const iconOnly = !children;
  const sizeClasses = KAESO_BUTTON_VARIANTS.size[size].classes;
  const btn = (
    <button
      ref={ref}
      type="button"
      {...(pressed !== undefined ? { "aria-pressed": pressed } : {})}
      className={cn(
        "relative inline-flex items-center justify-center whitespace-nowrap select-none transition-control focus-visible:z-10 cursor-pointer disabled:cursor-not-allowed",
        attached ? "focus-ring-segment" : "focus-ring",
        sizeClasses,
        iconOnly && ICON_ONLY[size],
        attached
          ? "rounded-none text-fg first:rounded-l-pill last:rounded-r-pill not-first:divider-l hover:bg-fill-1 active:bg-fill-2 aria-pressed:bg-fill-2 disabled:text-fg-disabled"
          : "rounded-pill material-secondary aria-pressed:bg-fill-2",
        className,
      )}
      {...props}
    >
      {icon && <Icon icon={icon} size={BUTTON_ICON_SIZE[size]} />}
      {children}
    </button>
  );
  if (iconOnly) warnMissingName("ButtonGroup.Item", !!props["aria-label"]);
  if (iconOnly && props["aria-label"]) return <Tooltip content={props["aria-label"]}>{btn}</Tooltip>;
  return btn;
});
Item.displayName = "ButtonGroup.Item";

export const ButtonGroup = Object.assign(Root, { Item });
