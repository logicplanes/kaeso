import { createContext, forwardRef, useContext, type AnchorHTMLAttributes, type ComponentType, type ReactNode, type Ref } from "react";

export type LinkComponentProps = AnchorHTMLAttributes<HTMLAnchorElement> & { href: string; ref?: Ref<HTMLAnchorElement> };
export type LinkComponent = ComponentType<LinkComponentProps>;

const DefaultLink = forwardRef<HTMLAnchorElement, LinkComponentProps>(function DefaultLink(props, ref) {
  return <a ref={ref} {...props} />;
});

const LinkContext = createContext<LinkComponent>(DefaultLink);

export function LinkProvider({ component, children }: { component: LinkComponent; children: ReactNode }) {
  return <LinkContext.Provider value={component}>{children}</LinkContext.Provider>;
}
LinkProvider.displayName = "LinkProvider";

export function useLinkComponent(): LinkComponent {
  return useContext(LinkContext);
}

export function isExternalHref(href: string): boolean {
  return /^(?:https?:)?\/\//i.test(href) || href.startsWith("mailto:");
}
