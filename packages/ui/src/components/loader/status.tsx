import { forwardRef, type HTMLAttributes } from "react";
import { cn } from "../../utils/cn";
import { KAESO_SPINNER_VARIANTS, Spinner, useDelayedLoading } from "./loader";
import { useUiText } from "../../i18n/context";

export const KAESO_LOADER_VARIANTS = {
  size: KAESO_SPINNER_VARIANTS.size,
  tone: {
    secondary: { classes: "text-fg-secondary", description: "Secondary text (default)" },
    tertiary: { classes: "text-fg-tertiary", description: "Tertiary text, the quietest: a page loading" },
    current: { classes: "", description: "The text color around it" },
  },
} as const;
export const KAESO_LOADER_DEFAULT_VARIANTS = { size: "sm", tone: "secondary" } as const;

export type LoaderSize = keyof typeof KAESO_LOADER_VARIANTS.size;
export type LoaderTone = keyof typeof KAESO_LOADER_VARIANTS.tone;

export interface LoaderProps extends HTMLAttributes<HTMLSpanElement> {
  size?: LoaderSize | undefined;
  tone?: LoaderTone | undefined;
  label?: string | undefined;
  hideLabel?: boolean | undefined;
  loading?: boolean | undefined;
  delay?: boolean | number | undefined;
  minDuration?: number | undefined;
}

export const Loader = forwardRef<HTMLSpanElement, LoaderProps>(function Loader(
  { size = "sm", tone, label, hideLabel, loading = true, delay, minDuration, className, ...props },
  ref,
) {
  const t = useUiText();
  const shown = useDelayedLoading(loading, delay === true ? undefined : delay || 0, minDuration ?? (delay ? undefined : 0));
  return (
    <span
      ref={ref}
      role="status"
      className={cn("inline-flex items-center gap-8 text-body", KAESO_LOADER_VARIANTS.tone[tone ?? "secondary"].classes, className)}
      {...props}
    >
      <span className={shown ? "contents" : "contents invisible"}>
        <Spinner size={size} />
        <span className={cn(!label || hideLabel ? "sr-only" : undefined)}>{label ?? t("loader.loading", "Loading")}</span>
      </span>
    </span>
  );
});
Loader.displayName = "Loader";
