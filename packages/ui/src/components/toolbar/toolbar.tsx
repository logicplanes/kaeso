import { forwardRef, type HTMLAttributes, type KeyboardEvent } from "react";
import { cn } from "../../utils/cn.js";
import { useInset } from "../../utils/inset.js";
import { arrowFocus } from "../../utils/arrow-focus.js";

export const KAESO_TOOLBAR_VARIANTS = {
  size: {
    md: { classes: "min-h-48 gap-8 px-24 py-6", description: "48px, 24px side padding, under the page header (default)" },
    inset: { classes: "min-h-48 gap-8 px-0 py-6", description: "48px without side padding, inside padded content" },
  },
} as const;
export const KAESO_TOOLBAR_DEFAULT_VARIANTS = { size: "md" } as const;

export interface ToolbarProps extends HTMLAttributes<HTMLDivElement> {
  size?: keyof typeof KAESO_TOOLBAR_VARIANTS.size | undefined;
  "aria-label": string;
}

const Root = forwardRef<HTMLDivElement, ToolbarProps>(function Toolbar({ size: sizeProp, className, onKeyDown, ...props }, ref) {
  const inset = useInset();
  const size = sizeProp ?? (inset ? "inset" : "md");
  return (
    <div
      ref={ref}
      role="toolbar"
      aria-orientation="horizontal"
      className={cn("flex shrink-0 flex-wrap items-center", KAESO_TOOLBAR_VARIANTS.size[size].classes, className)}
      onKeyDown={(e: KeyboardEvent<HTMLDivElement>) => {
        onKeyDown?.(e);
        arrowFocus(e);
      }}
      {...props}
    />
  );
});
Root.displayName = "Toolbar";

const Spacer = forwardRef<HTMLSpanElement>(function ToolbarSpacer(_props, ref) {
  return <span ref={ref} aria-hidden className="flex-1" />;
});
Spacer.displayName = "Toolbar.Spacer";

export const Toolbar = Object.assign(Root, { Spacer });
