import { createFileRoute } from "@tanstack/react-router";
import { InstanceUpdatesPage } from "~/features/instance/updates-page";

export const Route = createFileRoute("/_app/settings/instance/updates")({ component: InstanceUpdatesPage });
