import { createFileRoute } from "@tanstack/react-router";
import { EmailConfirmPage } from "~/features/public/email-confirm-page";

export const Route = createFileRoute("/me/email/confirm")({ component: EmailConfirmPage });
