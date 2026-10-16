import { createFileRoute } from "@tanstack/react-router";
import { SessionsPage } from "~/features/account/sessions-page";

export const Route = createFileRoute("/_app/settings/account/sessions")({ component: SessionsPage });
