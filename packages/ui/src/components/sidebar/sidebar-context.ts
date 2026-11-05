import { createContext } from "react";

export const SidebarFloatingContext = createContext(false);
export const SidebarSheetContext = createContext<{ close: () => void; peek: boolean } | null>(null);
