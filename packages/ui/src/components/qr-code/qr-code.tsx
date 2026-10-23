import { forwardRef, useMemo } from "react";
import { encode } from "uqr";
import { cn } from "../../utils/cn.js";

export const KAESO_QR_CODE_VARIANTS = {
  size: {
    md: { classes: "size-160", description: "160px: setup dialogs (default)" },
    lg: { classes: "size-200", description: "200px: a page of its own" },
  },
} as const;
export const KAESO_QR_CODE_DEFAULT_VARIANTS = { size: "md" } as const;

export interface QRCodeProps {
  value: string;
  label: string;
  size?: keyof typeof KAESO_QR_CODE_VARIANTS.size | undefined;
  className?: string | undefined;
}

export const QRCode = forwardRef<SVGSVGElement, QRCodeProps>(function QRCode({ value, label, size = "md", className }, ref) {
  const { path, extent } = useMemo(() => {
    const { data, size: n } = encode(value, { ecc: "M", border: 2 });
    let d = "";
    data.forEach((row, y) => row.forEach((dark, x) => dark && (d += `M${x} ${y}h1v1h-1z`)));
    return { path: d, extent: n };
  }, [value]);
  return (
    <svg
      ref={ref}
      viewBox={`0 0 ${extent} ${extent}`}
      role="img"
      aria-label={label}
      shapeRendering="crispEdges"
      className={cn("block shrink-0 rounded-well bg-scan text-on-scan shadow-ring", KAESO_QR_CODE_VARIANTS.size[size].classes, className)}
    >
      <path d={path} fill="currentColor" />
    </svg>
  );
});
QRCode.displayName = "QRCode";
