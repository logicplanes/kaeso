import { forwardRef, type AnchorHTMLAttributes, type ReactNode } from "react";
import { cn } from "../../utils/cn";
import { isExternalHref, useLinkComponent } from "../../utils/link-provider";
import { Icon } from "../icon/icon";
import { icons } from "../../icons/vocabulary";
import { useUiText } from "../../i18n/context";

export const KAESO_LINK_VARIANTS = {
  variant: {
    inline: {
      classes: "text-inherit underline decoration-line-strong decoration-1 underline-offset-3 hover:decoration-current",
      description: "Inside prose: text color with a quiet underline that turns solid on hover",
    },
    quiet: {
      classes: "text-fg-secondary no-underline hover:text-fg",
      description: "Navigation-style links in footers and metadata; no underline until hover",
    },
    standalone: {
      classes: "font-medium text-fg no-underline hover:underline hover:decoration-line-strong hover:underline-offset-3",
      description: "A link on its own line (“Team overview ›”)",
    },
  },
} as const;
export const KAESO_LINK_DEFAULT_VARIANTS = { variant: "inline" } as const;

export interface LinkProps extends Omit<AnchorHTMLAttributes<HTMLAnchorElement>, "href"> {
  href: string;
  variant?: keyof typeof KAESO_LINK_VARIANTS.variant | undefined;
  external?: boolean | undefined;
  children: ReactNode;
}

export const Link = forwardRef<HTMLAnchorElement, LinkProps>(function Link({ href, variant = "inline", external, className, children, ...props }, ref) {
  const LinkComp = useLinkComponent();
  const t = useUiText();
  const isExternal = external ?? isExternalHref(href);
  return (
    <LinkComp
      ref={ref}
      href={href}
      className={cn(
        "relative -mx-2 inline-flex items-baseline gap-2 rounded-code px-2 transition-control focus-ring tap-target",
        KAESO_LINK_VARIANTS.variant[variant].classes,
        className,
      )}
      {...(isExternal ? { target: "_blank", rel: "noopener noreferrer" } : {})}
      {...props}
    >
      {children}
      {isExternal && <Icon icon={icons.externalLink} size="xs" className="self-center" />}
      {isExternal && <span className="sr-only">{t("link.opensInNewTab", " (opens in a new tab)")}</span>}
    </LinkComp>
  );
});
Link.displayName = "Link";
