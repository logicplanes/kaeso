import { forwardRef, type AnchorHTMLAttributes, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { isExternalHref, useLinkComponent } from "../../utils/link-provider.js";
import { Icon } from "../icon/icon.js";
import { icons } from "../../icons/vocabulary.js";
import { useUiText } from "../../i18n/context.js";

export const KAESO_LINK_VARIANTS = {
  variant: {
    inline: {
      classes: "text-inherit underline decoration-fg-tertiary decoration-1 underline-offset-3 hover:decoration-current",
      description: "Inside prose: text color with a grey underline that turns to the text color on hover",
    },
    quiet: {
      classes: "text-fg-secondary no-underline hover:text-fg",
      description: "Navigation-style links in footers and metadata; no underline until hover",
    },
    standalone: {
      classes: "font-medium text-fg no-underline hover:underline hover:decoration-fg-tertiary hover:underline-offset-3",
      description: "A link on its own line (“Team overview ›”)",
    },
    heading: {
      classes: "group/heading text-inherit no-underline",
      description: "A heading's link to itself: the heading's own look, with a # beside it on hover and keyboard focus",
    },
    card: {
      classes: "mx-0 flex flex-col gap-4 rounded-well bg-fill-1 px-16 py-12 text-fg no-underline hover:bg-fill-2",
      description: "A page to read next: a filled well with the page's name and one sentence about it",
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
        "relative -mx-2 box-decoration-clone rounded-code px-2 transition-control focus-ring tap-target",
        KAESO_LINK_VARIANTS.variant[variant].classes,
        variant === "card" && isExternal && "pe-36",
        className,
      )}
      {...(isExternal ? { target: "_blank", rel: "noopener noreferrer" } : {})}
      {...props}
    >
      {children}
      {isExternal && (
        <span aria-hidden="true" className={cn("whitespace-nowrap", variant === "card" && "absolute end-16 top-12")}>
          {"\u2060"}
          <Icon icon={icons.externalLink} size="xs" className="ms-2 inline-block align-middle" />
        </span>
      )}
      {variant === "heading" && (
        <span
          aria-hidden="true"
          className="ms-8 inline-block text-fg-tertiary opacity-0 transition-control group-hover/heading:opacity-100 group-focus-visible/heading:opacity-100"
        >
          #
        </span>
      )}
      {isExternal && <span className="sr-only">{t("link.opensInNewTab", " (opens in a new tab)")}</span>}
    </LinkComp>
  );
});
Link.displayName = "Link";
