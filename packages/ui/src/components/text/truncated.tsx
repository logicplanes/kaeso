import { forwardRef, useLayoutEffect, useRef, useState, type HTMLAttributes, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { mergeRefs } from "../../utils/merge-refs.js";
import { useTruncated } from "../../hooks/use-truncated.js";
import { Tooltip } from "../tooltip/tooltip.js";

const FOCUSABLE = "a[href], button, input, select, textarea, [tabindex]";

export interface TruncatedTextProps extends Omit<HTMLAttributes<HTMLSpanElement>, "children"> {
  children: ReactNode;
  className?: string | undefined;
  wrap?: boolean | undefined;
}

export const TruncatedText = forwardRef<HTMLSpanElement, TruncatedTextProps>(function TruncatedText({ children, className, wrap = false, ...props }, ref) {
  const inner = useRef<HTMLSpanElement | null>(null);
  const cut = useTruncated(inner, !wrap);
  const [full, setFull] = useState("");
  const [nested, setNested] = useState(false);
  useLayoutEffect(() => {
    const el = inner.current;
    if (!cut || !el) return;
    setFull(typeof children === "string" ? children : (el.textContent ?? "").trim());
    setNested(!!el.querySelector(FOCUSABLE));
  }, [cut, children]);
  const el = (
    <span
      {...props}
      ref={mergeRefs(ref, inner)}
      className={cn(wrap ? "text-pretty" : "truncate", cut && full && !nested && "rounded-code focus-ring", className)}
      {...(cut && full && !nested ? { tabIndex: 0 } : {})}
    >
      {children}
    </span>
  );
  return cut && full ? (
    <Tooltip content={full} shape="multiline">
      {el}
    </Tooltip>
  ) : (
    el
  );
});
TruncatedText.displayName = "TruncatedText";
