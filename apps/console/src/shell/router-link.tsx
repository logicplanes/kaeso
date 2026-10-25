import { forwardRef } from "react";
import { Link } from "@tanstack/react-router";
import type { LinkComponentProps } from "@logicplanes/kaeso-ui";
import { internalPath } from "~/auth/next";

const CURRENT_ONLY = { exact: true } as const;
const OUTSIDE = /^(?:https?:\/\/|mailto:|otpauth:\/\/|cursor:\/\/|vscode:|#)/i;

export const RouterLink = forwardRef<HTMLAnchorElement, LinkComponentProps>(function RouterLink({ href, target, children, ...props }, ref) {
  const outside = OUTSIDE.test(href);
  const path = outside ? null : internalPath(href);
  const opens = target ? { target } : {};
  if (path === null) {
    return (
      <a ref={ref} {...(outside ? { href } : {})} {...opens} {...props}>
        {children}
      </a>
    );
  }
  return (
    <Link ref={ref} to={path} activeOptions={CURRENT_ONLY} {...opens} {...props}>
      {children}
    </Link>
  );
});
