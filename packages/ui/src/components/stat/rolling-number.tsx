import { forwardRef, useEffect, useRef, type CSSProperties } from "react";
import { cn } from "../../utils/cn.js";

export const KAESO_ROLLING_NUMBER_VARIANTS = {} as const;
export const KAESO_ROLLING_NUMBER_DEFAULT_VARIANTS = {} as const;

export interface RollingNumberProps {
  value: string;
  className?: string | undefined;
}

const DIGIT = /[0-9]/;

export const RollingNumber = forwardRef<HTMLSpanElement, RollingNumberProps>(function RollingNumber({ value, className }, ref) {
  const painted = useRef(false);
  useEffect(() => {
    painted.current = true;
  }, []);
  const chars = [...value];
  const enter = painted.current && "digit-enter";
  return (
    <span ref={ref} className={cn("inline-flex whitespace-nowrap tabular", className)}>
      <span className="sr-only">{value}</span>
      <span aria-hidden className="inline-flex">
        {chars.map((c, i) => {
          const place = chars.length - i;
          return DIGIT.test(c) ? (
            <span key={`d${place}`} className={cn("digit-window", enter)}>
              <span className="digit-strip" style={{ "--k-digit": Number(c) } as CSSProperties} />
            </span>
          ) : (
            <span key={`c${place}${c}`} data-char={c} className={cn("digit-char", enter)} />
          );
        })}
      </span>
    </span>
  );
});
RollingNumber.displayName = "RollingNumber";
