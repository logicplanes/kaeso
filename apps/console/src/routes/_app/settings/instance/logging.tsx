import { createFileRoute } from "@tanstack/react-router";
import { InstanceLoggingPage } from "~/features/instance/logging-page";

export const Route = createFileRoute("/_app/settings/instance/logging")({ component: InstanceLoggingPage });
