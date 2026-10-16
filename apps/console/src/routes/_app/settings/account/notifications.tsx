import { createFileRoute } from "@tanstack/react-router";
import { NotificationsPage } from "~/features/account/notifications-page";

export const Route = createFileRoute("/_app/settings/account/notifications")({ component: NotificationsPage });
