export const MENU_POPUP =
  "z-50 flex min-w-220 max-w-360 flex-col overflow-y-auto overscroll-contain rounded-menu bg-raised p-6 text-body text-fg shadow-raised outline-none popup-motion scrollbar-quiet max-h-(--available-height)";

export const MENU_ITEM =
  "group/item relative flex min-h-34 w-full cursor-default items-center gap-10 rounded-item px-10 text-left text-body font-medium text-fg outline-none select-none coarse:min-h-44 highlighted:bg-item-hover active:bg-item-pressed disabled:pointer-events-none disabled:text-fg-disabled";

export const MENU_ICON = "text-fg-secondary group-data-highlighted/item:text-fg group-data-disabled/item:text-fg-disabled";

export const MENU_DANGER = "text-danger-text highlighted:bg-danger-tint";

export const MENU_LABEL = "flex h-28 items-center px-10 text-caption font-medium text-fg-secondary";

export const MENU_SEPARATOR = "mx-10 my-4 h-1 shrink-0 bg-line-subtle";

export const MENU_HINT =
  "ml-auto flex shrink-0 items-center gap-4 pl-16 text-caption font-medium text-fg-tertiary tabular group-data-highlighted/item:text-fg-secondary";
