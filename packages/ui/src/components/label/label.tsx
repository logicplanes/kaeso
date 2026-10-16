import { forwardRef, type LabelHTMLAttributes, type ReactNode } from "react";
import { cn } from "../../utils/cn";
import { useUiText } from "../../i18n/context";

export const KAESO_LABEL_VARIANTS = {
  size: {
    md: { classes: "text-body font-medium", description: "14/500: field labels" },
    sm: { classes: "text-small font-medium", description: "13/500: compact forms" },
  },
} as const;
export const KAESO_LABEL_DEFAULT_VARIANTS = { size: "md" } as const;

export interface LabelProps extends LabelHTMLAttributes<HTMLLabelElement> {
  size?: keyof typeof KAESO_LABEL_VARIANTS.size | undefined;
  optional?: boolean | undefined;
  children: ReactNode;
}

export const Label = forwardRef<HTMLLabelElement, LabelProps>(function Label({ size = "md", optional, className, children, ...props }, ref) {
  const t = useUiText();
  return (
    <label ref={ref} className={cn("inline-flex items-baseline gap-6 text-fg", KAESO_LABEL_VARIANTS.size[size].classes, className)} {...props}>
      {children}
      {optional && <span className="text-small font-regular text-fg-tertiary">{t("label.optional", "Optional")}</span>}
    </label>
  );
});
Label.displayName = "Label";
