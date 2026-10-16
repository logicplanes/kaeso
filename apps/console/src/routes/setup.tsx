import { createFileRoute } from "@tanstack/react-router";
import { SetupPage } from "~/features/public/setup-page";

export const Route = createFileRoute("/setup")({ component: SetupPage });
