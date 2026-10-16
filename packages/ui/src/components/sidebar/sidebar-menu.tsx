import { useEffect, useLayoutEffect, useState, type ReactNode } from "react";
import { mergeProps } from "@base-ui/react/merge-props";
import { Menu } from "../menu/menu.js";
import type { TriggerProps, TriggerPropsStore } from "../tooltip/trigger-props.js";

export interface SidebarMenuRootProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  triggerProps: TriggerPropsStore;
  side: "top" | "bottom";
  header?: ReactNode | undefined;
  footer?: ReactNode | undefined;
  children: ReactNode;
}

export function SidebarMenuRoot({ open, onOpenChange, triggerProps, side, header, footer, children }: SidebarMenuRootProps) {
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);
  return (
    <Menu open={open && mounted} onOpenChange={onOpenChange}>
      <Menu.Trigger>
        <HandOver store={triggerProps} />
      </Menu.Trigger>
      <Menu.Content side={side} align="start" {...(header ? { header } : {})} {...(footer ? { footer } : {})}>
        {children}
      </Menu.Content>
    </Menu>
  );
}
SidebarMenuRoot.displayName = "Sidebar.MenuRoot";

function HandOver({ store, ...props }: TriggerProps & { store: TriggerPropsStore }) {
  useLayoutEffect(() => store.set(props));
  useLayoutEffect(() => () => store.set(null), [store]);
  return null;
}
HandOver.displayName = "Sidebar.MenuHandOver";

export function mergeTriggerProps(given: TriggerProps, own: object): object {
  const { ref: _ref, ...props } = given;
  return mergeProps<"button">(props, own);
}
