import { forwardRef } from "react";
import { Link } from "@tanstack/react-router";
import type { LinkComponentProps } from "@logicplanes/kaeso-ui";

export const RouterLink = forwardRef<HTMLAnchorElement, LinkComponentProps>(function RouterLink({ href, target, children, ...props }, ref) {
  if (/^(?:[a-z][a-z0-9+.-]*:)?\/\//i.test(href) || href.startsWith("mailto:") || href.startsWith("#")) {
    return (
      <a ref={ref} href={href} {...(target ? { target } : {})} {...props}>
        {children}
      </a>
    );
  }
  return (
    <Link ref={ref} to={href} {...(target ? { target } : {})} {...props}>
      {children}
    </Link>
  );
});
