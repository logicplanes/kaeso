import { createFileRoute } from "@tanstack/react-router";
import { InstanceGeneralPage } from "~/features/instance/general-page";

export const Route = createFileRoute("/_app/settings/instance/general")({ component: InstanceGeneralPage });
