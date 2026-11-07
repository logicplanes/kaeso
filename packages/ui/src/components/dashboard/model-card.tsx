import { forwardRef, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";

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
        <dl className="mt-16 grid grid-cols-2 gap-x-16 gap-y-8">
          {facts.map((f, i) => (
            <div key={i} className="flex min-w-0 flex-col gap-2">
              <dt className="truncate text-caption text-fg-tertiary">{f.label}</dt>
              <dd className="truncate text-small font-medium text-fg tabular">{f.value}</dd>
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
    className?: string | undefined;
  }
>(function Announcement({ eyebrow, title, description, actions, art, className }, ref) {
  return (
    <section ref={ref} className={cn("relative flex min-w-0 flex-col overflow-hidden bg-raised md:flex-row", className)}>
      <div className="flex min-w-0 flex-1 flex-col justify-center px-20 py-24">
        {eyebrow && <span className="text-small font-medium text-fg-tertiary">{eyebrow}</span>}
        <h2 className="mt-4 text-title font-semibold text-pretty text-fg">{title}</h2>
        {description && <p className="mt-6 line-clamp-2 max-w-640 text-body text-fg-secondary">{description}</p>}
        {actions && <div className="mt-16 flex flex-wrap items-center gap-8">{actions}</div>}
      </div>
      {art && <div className="hidden min-h-140 w-320 shrink-0 md:block">{art}</div>}
    </section>
  );
});
Announcement.displayName = "Announcement";
