import { createFileRoute } from "@tanstack/react-router";
import { MagicLinkPage } from "~/features/public/magic-link-page";
import { internalPath } from "~/auth/next";

const magicLinkSearch = (search: Record<string, unknown>): { next?: string } =>
  typeof search.next === "string" && internalPath(search.next) ? { next: search.next } : {};

export const Route = createFileRoute("/auth/magic-link")({ validateSearch: magicLinkSearch, component: MagicLink });

function MagicLink() {
  return <MagicLinkPage next={Route.useSearch({ select: magicLinkSearch, structuralSharing: true }).next} />;
}
