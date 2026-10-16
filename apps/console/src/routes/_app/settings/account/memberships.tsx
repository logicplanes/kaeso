import { createFileRoute } from "@tanstack/react-router";
import { MembershipsPage } from "~/features/account/memberships-page";

export const Route = createFileRoute("/_app/settings/account/memberships")({ component: MembershipsPage });
