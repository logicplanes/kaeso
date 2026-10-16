import { createFileRoute } from "@tanstack/react-router";
import { InstanceModelSourcesPage } from "~/features/instance/model-sources-page";

export const Route = createFileRoute("/_app/settings/instance/model-sources")({ component: InstanceModelSourcesPage });
