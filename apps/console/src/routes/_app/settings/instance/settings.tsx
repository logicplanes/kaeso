import { createFileRoute } from "@tanstack/react-router";
import { InstanceSettingsPage } from "~/features/instance/settings-page";

export const Route = createFileRoute("/_app/settings/instance/settings")({ component: InstanceSettingsPage });
