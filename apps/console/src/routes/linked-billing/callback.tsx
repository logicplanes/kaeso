import { createFileRoute } from "@tanstack/react-router";
import { LinkCallbackPage } from "~/features/public/link-callback-page";

export const Route = createFileRoute("/linked-billing/callback")({ component: LinkCallbackPage });
