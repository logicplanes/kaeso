import { createFileRoute } from "@tanstack/react-router";
import { InstanceSystemPage } from "~/features/instance/system-page";

export const Route = createFileRoute("/_app/settings/instance/system")({ component: InstanceSystemPage });
