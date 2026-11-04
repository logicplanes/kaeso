import { forwardRef, useId, type HTMLAttributes, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";

export const KAESO_FLOW_WORKSPACE_VARIANTS = {
  width: {
    default: { classes: "lg:w-400", description: "A 400px inspector from lg (default)" },
    wide: { classes: "lg:w-480", description: "A 480px inspector from lg, for forms with long values" },
  },
} as const;
export const KAESO_FLOW_WORKSPACE_DEFAULT_VARIANTS = { width: "default" } as const;

export interface FlowWorkspaceProps extends HTMLAttributes<HTMLDivElement> {
  children: ReactNode;
  panel?: ReactNode | undefined;
  panelLabel?: string | undefined;
  width?: keyof typeof KAESO_FLOW_WORKSPACE_VARIANTS.width | undefined;
}

const Root = forwardRef<HTMLDivElement, FlowWorkspaceProps>(function FlowWorkspace(
  { children, panel, panelLabel, width = "default", className, ...props },
  ref,
) {
  return (
    <div
      ref={ref}
      data-flow-workspace=""
      className={cn("flex min-h-0 flex-1 flex-col overflow-y-auto overscroll-contain scrollbar-quiet lg:flex-row lg:overflow-hidden", className)}
      {...props}
    >
      <div data-flow-workspace-canvas="" className="relative h-400 shrink-0 lg:h-auto lg:min-h-0 lg:min-w-0 lg:flex-1">
        {children}
      </div>
      {panel && (
        <aside
          {...(panelLabel ? { "aria-label": panelLabel } : {})}
          data-flow-workspace-panel=""
          className={cn(
            "relative shrink-0 max-lg:divider-t lg:min-h-0 lg:overflow-y-auto lg:overscroll-contain lg:scrollbar-quiet lg:divider-l",
            KAESO_FLOW_WORKSPACE_VARIANTS.width[width].classes,
          )}
        >
          {panel}
        </aside>
      )}
    </div>
  );
});
Root.displayName = "FlowWorkspace";

export interface FlowWorkspacePanelProps extends Omit<HTMLAttributes<HTMLElement>, "title"> {
  title: ReactNode;
  description?: ReactNode | undefined;
  actions?: ReactNode | undefined;
  children?: ReactNode | undefined;
}

export const FlowWorkspacePanel = forwardRef<HTMLElement, FlowWorkspacePanelProps>(function FlowWorkspacePanel(
  { title, description, actions, children, className, ...props },
  ref,
) {
  const id = `k-flow-panel-${useId().replace(/[^\w-]/g, "")}`;
  return (
    <section ref={ref} aria-labelledby={id} className={cn("flex flex-col px-24 pt-20 pb-40", className)} {...props}>
      <header className="flex min-h-32 items-center justify-between gap-12">
        <h2 id={id} className="min-w-0 text-title-sm font-semibold text-balance text-fg">
          {title}
        </h2>
        {actions && <div className="flex shrink-0 items-center gap-8">{actions}</div>}
      </header>
      {description && <p className="mt-4 text-small text-fg-secondary">{description}</p>}
      {children && <div className="mt-20 flex flex-col gap-20">{children}</div>}
    </section>
  );
});
FlowWorkspacePanel.displayName = "FlowWorkspace.Panel";

export const FlowWorkspace = Object.assign(Root, { Panel: FlowWorkspacePanel });
