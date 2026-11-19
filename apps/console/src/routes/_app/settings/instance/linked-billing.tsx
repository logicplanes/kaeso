import { createFileRoute } from "@tanstack/react-router";
import { InstanceLinkedBillingPage } from "~/features/instance/linked-billing-page";

export const Route = createFileRoute("/_app/settings/instance/linked-billing")({ component: InstanceLinkedBillingPage });
