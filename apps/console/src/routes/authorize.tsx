import { createFileRoute } from "@tanstack/react-router";
import { AuthorizePage } from "~/features/public/authorize-page";

export const Route = createFileRoute("/authorize")({ component: AuthorizePage });
