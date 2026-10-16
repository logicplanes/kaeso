import { createFileRoute } from "@tanstack/react-router";
import { AppearancePage } from "~/features/account/appearance-page";

export const Route = createFileRoute("/_app/settings/account/appearance")({ component: AppearancePage });
