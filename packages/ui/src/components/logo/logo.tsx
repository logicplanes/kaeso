import { forwardRef, type SVGAttributes } from "react";
import { cn } from "../../utils/cn";
import * as geometry from "./paths";

export const KAESO_LOGO_VARIANTS = {
  markSize: {
    sm: { classes: "h-20", description: "20px high: the minimum in interfaces" },
    md: { classes: "h-24", description: "24px: compact navigation" },
    lg: { classes: "h-32", description: "32px" },
    xl: { classes: "h-48", description: "48px: empty and setup screens" },
  },
  lockupSize: {
    sm: { classes: "h-28", description: "28px high (112px wide): the minimum" },
    md: { classes: "h-32", description: "32px: headers" },
    lg: { classes: "h-40", description: "40px: auth and setup screens" },
  },
} as const;
export const KAESO_LOGO_DEFAULT_VARIANTS = { markSize: "md", lockupSize: "sm" } as const;

type Geometry = { readonly viewBox: string; readonly paths: readonly string[] };
interface BaseLogoProps extends Omit<SVGAttributes<SVGSVGElement>, "children"> {
  title?: string | undefined;
  decorative?: boolean | undefined;
}

function makeSvg(name: string, g: Geometry, defaultTitle: string, base: string) {
  const C = forwardRef<SVGSVGElement, BaseLogoProps & { sizeClass: string }>(function LogoSvg(
    { title = defaultTitle, decorative, sizeClass, className, ...props },
    ref,
  ) {
    return (
      <svg
        ref={ref}
        viewBox={g.viewBox}
        fill="currentColor"
        xmlns="http://www.w3.org/2000/svg"
        className={cn("block w-auto shrink-0", base, sizeClass, className)}
        {...(decorative ? { "aria-hidden": true, focusable: false } : { role: "img", "aria-label": title })}
        {...props}
      >
        {g.paths.map((d) => (
          <path key={d} d={d} />
        ))}
      </svg>
    );
  });
  C.displayName = name;
  return C;
}

const MarkSvg = makeSvg("Logo.Mark", geometry.mark, "Kaeso", "");
const IconSvg = makeSvg("Logo.Icon", geometry.markIcon, "Kaeso", "");
const LockupSvg = makeSvg("Logo.Lockup", geometry.lockup, "Kaeso", "");
const WordmarkSvg = makeSvg("Logo.Wordmark", geometry.wordmark, "Kaeso", "");

export interface LogoMarkProps extends BaseLogoProps {
  size?: keyof typeof KAESO_LOGO_VARIANTS.markSize | undefined;
}
export interface LogoLockupProps extends BaseLogoProps {
  size?: keyof typeof KAESO_LOGO_VARIANTS.lockupSize | undefined;
}

const Mark = forwardRef<SVGSVGElement, LogoMarkProps>(function Mark({ size = "md", ...props }, ref) {
  return <MarkSvg ref={ref} sizeClass={KAESO_LOGO_VARIANTS.markSize[size].classes} {...props} />;
});
Mark.displayName = "Logo.Mark";

const IconMark = forwardRef<SVGSVGElement, LogoMarkProps>(function IconMark({ size = "md", ...props }, ref) {
  return <IconSvg ref={ref} sizeClass={KAESO_LOGO_VARIANTS.markSize[size].classes} {...props} />;
});
IconMark.displayName = "Logo.Icon";

const Lockup = forwardRef<SVGSVGElement, LogoLockupProps>(function Lockup({ size = "sm", ...props }, ref) {
  return <LockupSvg ref={ref} sizeClass={KAESO_LOGO_VARIANTS.lockupSize[size].classes} {...props} />;
});
Lockup.displayName = "Logo.Lockup";

export interface LogoWordmarkProps extends BaseLogoProps {
  size?: keyof typeof KAESO_LOGO_VARIANTS.lockupSize | "nav" | undefined;
}

const WORDMARK_TIGHT = "72 69.3 881.9 177.7";

const Wordmark = forwardRef<SVGSVGElement, LogoWordmarkProps>(function Wordmark({ size = "sm", title = "Kaeso", decorative, className, style, ...props }, ref) {
  if (size === "nav") {
    return (
      <svg
        ref={ref}
        viewBox={WORDMARK_TIGHT}
        fill="currentColor"
        xmlns="http://www.w3.org/2000/svg"
        className={cn("block shrink-0", className)}
        style={{ width: 60, height: 12, ...style }}
        {...(decorative ? { "aria-hidden": true, focusable: false } : { role: "img", "aria-label": title })}
        {...props}
      >
        {geometry.wordmark.paths.map((d) => (
          <path key={d} d={d} />
        ))}
      </svg>
    );
  }
  return (
    <WordmarkSvg
      ref={ref}
      sizeClass={size === "sm" ? "h-24" : KAESO_LOGO_VARIANTS.lockupSize[size].classes}
      title={title}
      {...(decorative ? { decorative } : {})}
      {...(className ? { className } : {})}
      {...(style ? { style } : {})}
      {...props}
    />
  );
});
Wordmark.displayName = "Logo.Wordmark";

export const Logo = Object.assign(
  forwardRef<SVGSVGElement, LogoLockupProps>(function Logo(props, ref) {
    return <Lockup ref={ref} {...props} />;
  }),
  { Mark, Icon: IconMark, Lockup, Wordmark },
);
Logo.displayName = "Logo";
