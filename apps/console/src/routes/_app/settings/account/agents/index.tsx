import { createFileRoute } from "@tanstack/react-router";
import { AgentsPage } from "~/features/account/agents-page";

export const Route = createFileRoute("/_app/settings/account/agents/")({ component: AgentsPage });
