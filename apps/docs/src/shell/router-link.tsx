import { forwardRef } from "react";
import { Link } from "@tanstack/react-router";
import type { LinkComponentProps } from "@logicplanes/kaeso-ui";

const EXACT = { exact: true, includeHash: false, includeSearch: false } as const;

export const RouterLink = forwardRef<HTMLAnchorElement, LinkComponentProps>(function RouterLink({ href, target, children, ...props }, ref) {
  if (
    /^(?:[a-z][a-z0-9+.-]*:)?\/\//i.test(href) ||
    /^[a-z][a-z0-9+.-]*:/i.test(href) ||
    href.startsWith("#") ||
    href.startsWith("/assets/") ||
    target === "_blank"
  ) {
    return (
      <a ref={ref} href={href} {...(target ? { target } : {})} {...props}>
        {children}
      </a>
    );
  }
  const [path = "/", hash] = href.split("#");
  return (
    <Link ref={ref} to={path} {...(hash ? { hash } : {})} activeOptions={EXACT} {...props}>
      {children}
    </Link>
  );
});
