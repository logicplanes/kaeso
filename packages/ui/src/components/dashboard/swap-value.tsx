import { forwardRef, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { RollingNumber } from "../stat/rolling-number.js";

function useChanged(value: string): number {
  const last = useRef(value);
  const [count, setCount] = useState(0);
  useLayoutEffect(() => {
    if (last.current === value) return;
    last.current = value;
    setCount((c) => c + 1);
  }, [value]);
  return count;
}

export const SwapValue = forwardRef<HTMLSpanElement, { children: ReactNode; text: string; className: string }>(function SwapValue(
  { children, text, className },
  ref,
) {
  const changes = useChanged(text);
  if (text) return <RollingNumber ref={ref} value={text} className={className} />;
  return (
    <span ref={ref} key={changes} className={cn(className, changes > 0 && "value-swap-in")}>
      {children}
    </span>
  );
});
SwapValue.displayName = "SwapValue";

export const textOf = (n: ReactNode) => (typeof n === "string" || typeof n === "number" ? String(n) : "");
