import type { ReactNode } from "react";
import { cn } from "@logicplanes/kaeso-ui";

export function StateMatrix({ title, columns, children, className }: { title?: string; columns: string[]; children: ReactNode; className?: string }) {
  return (
    <section className={cn("mb-48", className)} data-matrix={title}>
      {title && <h2 className="mb-16 text-body font-semibold text-fg">{title}</h2>}
      <div className="grid items-center gap-x-24 gap-y-16" style={{ gridTemplateColumns: `120px repeat(${columns.length}, minmax(max-content, 1fr))` }}>
        <span />
        {columns.map((c) => (
          <span key={c} className="text-caption font-medium text-fg-tertiary">
            {c}
          </span>
        ))}
        {children}
      </div>
    </section>
  );
}

export function StateRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <>
      <span className="text-caption font-medium text-fg-secondary">{label}</span>
      {children}
    </>
  );
}

export function StateCell({ id, children, className }: { id?: string; children: ReactNode; className?: string }) {
  return (
    <div className={cn("flex min-h-40 items-center", className)} data-cell={id}>
      {children}
    </div>
  );
}

export function StateSection({ title, children, className }: { title: string; children: ReactNode; className?: string }) {
  return (
    <section className={cn("mb-48", className)} data-matrix={title}>
      <h2 className="mb-16 text-body font-semibold text-fg">{title}</h2>
      {children}
    </section>
  );
}

export const STATE_COLUMNS: { label: string; props: Record<string, unknown> }[] = [
  { label: "Rest", props: {} },
  { label: "Hover", props: { "data-force": "hover" } },
  { label: "Focus", props: { "data-force": "focus" } },
  { label: "Pressed", props: { "data-force": "pressed" } },
  { label: "Disabled", props: { disabled: true } },
];

export function Matrix<R extends { label: string }>({
  title,
  rows,
  columns = STATE_COLUMNS,
  render,
}: {
  title: string;
  rows: R[];
  columns?: { label: string; props: Record<string, unknown> }[];
  render: (row: R, props: Record<string, unknown>, column: string) => ReactNode;
}) {
  return (
    <StateMatrix title={title} columns={columns.map((c) => c.label)}>
      {rows.map((r) => (
        <StateRow key={r.label} label={r.label}>
          {columns.map((c) => (
            <StateCell key={c.label} id={`${r.label}-${c.label}`.toLowerCase().replace(/\s+/g, "-")}>
              {render(r, c.props, c.label)}
            </StateCell>
          ))}
        </StateRow>
      ))}
    </StateMatrix>
  );
}

export function forceIn(selector: string, force: string) {
  return (el: HTMLElement | null) => {
    el?.querySelector(selector)?.setAttribute("data-force", force);
  };
}
