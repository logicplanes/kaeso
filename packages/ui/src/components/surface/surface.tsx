import { forwardRef, type HTMLAttributes } from "react";
import { cn } from "../../utils/cn.js";

export const KAESO_SURFACE_VARIANTS = {
  kind: {
    panel: { classes: "bg-panel rounded-panel shadow-panel", description: "The content panel: radius 12, hairline ring" },
    menu: { classes: "bg-raised rounded-menu shadow-raised", description: "Menus, select and combobox lists: radius 16" },
    popover: { classes: "bg-raised rounded-popover shadow-raised", description: "Popovers and hover cards: radius 16" },
    dialog: { classes: "bg-raised rounded-dialog shadow-dialog", description: "Dialogs and the command palette: radius 20" },
    sheet: { classes: "bg-raised rounded-sheet shadow-dialog", description: "Sheets: radius 16 on exposed corners" },
    group: { classes: "bg-raised rounded-group shadow-xs", description: "Settings groups: radius 14, ring and a whisper of shadow" },
    well: { classes: "bg-fill-1 rounded-well", description: "Code, the key reveal, the JSON inspector: fill, no ring" },
    tooltip: { classes: "bg-raised rounded-tooltip shadow-raised", description: "Chart tooltips and rich tooltips: radius 12" },
  },
} as const;
export const KAESO_SURFACE_DEFAULT_VARIANTS = { kind: "panel" } as const;

export interface SurfaceProps extends HTMLAttributes<HTMLDivElement> {
  kind: keyof typeof KAESO_SURFACE_VARIANTS.kind;
}

export const Surface = forwardRef<HTMLDivElement, SurfaceProps>(function Surface({ kind, className, ...props }, ref) {
  return <div ref={ref} className={cn(KAESO_SURFACE_VARIANTS.kind[kind].classes, className)} {...props} />;
});
Surface.displayName = "Surface";
