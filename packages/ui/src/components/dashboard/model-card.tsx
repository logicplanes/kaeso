import { forwardRef, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { figures } from "../../utils/figures.js";
import { Icon } from "../icon/icon.js";
import { icons } from "../../icons/vocabulary.js";
import { useUiText } from "../../i18n/context.js";

export interface ModelCardProps {
  mark: ReactNode;
  name: string;
  model: string;
  badge?: ReactNode | undefined;
  detail?: ReactNode | undefined;
  facts?: { label: ReactNode; value: ReactNode }[] | undefined;
  action?: ReactNode | undefined;
  className?: string | undefined;
}

export const ModelCard = forwardRef<HTMLElement, ModelCardProps>(function ModelCard({ mark, name, model, badge, detail, facts, action, className }, ref) {
  return (
    <article ref={ref} className={cn("flex min-w-0 flex-col bg-raised px-20 pt-20 pb-20", className)}>
      <div className="flex items-start justify-between gap-12">
        {mark}
        {badge}
      </div>
      <h3 className="mt-16 truncate text-title-sm font-semibold text-fg">{name}</h3>
      <p className="mt-2 truncate mono text-small text-fg-tertiary">{model}</p>
      {detail && <p className="mt-8 line-clamp-2 text-small text-fg-secondary">{detail}</p>}
      {facts && facts.length > 0 && (
        <dl className="mt-16 grid grid-cols-2 gap-x-16 gap-y-8 pt-12 hairline-t">
          {facts.map((f, i) => (
            <div key={i} className="flex min-w-0 flex-col gap-2">
              <dt className="truncate text-caption text-fg-tertiary">{f.label}</dt>
              <dd className={cn("text-small font-medium wrap-break-word text-fg", figures(f.value))}>{f.value}</dd>
            </div>
          ))}
        </dl>
      )}
      {action && <div className="mt-auto flex pt-20">{action}</div>}
    </article>
  );
});
ModelCard.displayName = "ModelCard";

export const Announcement = forwardRef<
  HTMLElement,
  {
    eyebrow?: ReactNode | undefined;
    title: ReactNode;
    description?: ReactNode | undefined;
    actions?: ReactNode | undefined;
    art?: ReactNode | undefined;
    scheme?: "light" | "dark" | undefined;
    onDismiss?: (() => void) | undefined;
    className?: string | undefined;
  }
>(function Announcement({ eyebrow, title, description, actions, art, scheme, onDismiss, className }, ref) {
  const t = useUiText();
  return (
    <section
      ref={ref}
      {...(art && scheme ? { "data-theme": scheme } : {})}
      className={cn("relative flex min-w-0 flex-col justify-center overflow-hidden bg-raised px-20 py-24", className)}
    >
      {art && <div className="absolute inset-0">{art}</div>}
      <h2 className={cn("relative text-title-lg font-semibold text-pretty text-fg", onDismiss && "pr-32")}>{title}</h2>
      {eyebrow && <p className="relative mt-6 text-small font-medium text-fg-secondary">{eyebrow}</p>}
      {/* Two lines' room whatever the text, so the news keeps its size while it loads and whatever its line says. */}
      {description && <p className="relative mt-12 line-clamp-2 min-h-40 max-w-480 text-body text-fg-secondary">{description}</p>}
      {actions && <div className="relative mt-16 flex flex-wrap items-center gap-8">{actions}</div>}
      {onDismiss && (
        <button
          type="button"
          aria-label={t("banner.dismiss", "Dismiss")}
          onClick={onDismiss}
          className={cn(
            "absolute top-12 right-12 inline-flex size-24 cursor-pointer items-center justify-center rounded-pill transition-control tap-target focus-ring",
            // On artwork it sits on its own raised surface, like the actions' buttons, so it reads over any shape or light.
            art ? "material-secondary" : "text-fg-secondary hover:bg-fill-2 hover:text-fg active:bg-fill-3",
          )}
        >
          <Icon icon={icons.close} size="xs" />
        </button>
      )}
    </section>
  );
});
Announcement.displayName = "Announcement";
