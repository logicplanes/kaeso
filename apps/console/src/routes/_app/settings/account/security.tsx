import { createFileRoute } from "@tanstack/react-router";
import { SecurityPage } from "~/features/account/security-page";

export const Route = createFileRoute("/_app/settings/account/security")({ component: SecurityPage });
