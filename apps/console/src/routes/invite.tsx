import { createFileRoute } from "@tanstack/react-router";
import { InvitePage } from "~/features/public/invite-page";

export const Route = createFileRoute("/invite")({ component: InvitePage });
