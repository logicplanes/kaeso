import type { ReactNode } from "react";
import { Tooltip as BaseTooltip } from "@base-ui/react/tooltip";

export function TooltipProvider({ children, delay = 500 }: { children: ReactNode; delay?: number | undefined }) {
  return (
    <BaseTooltip.Provider delay={delay} closeDelay={0} timeout={400}>
      {children}
    </BaseTooltip.Provider>
  );
}
TooltipProvider.displayName = "TooltipProvider";
