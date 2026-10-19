import { createFileRoute } from "@tanstack/react-router";
import { EmailCancelPage } from "~/features/public/email-cancel-page";

export const Route = createFileRoute("/me/email/cancel")({ component: EmailCancelPage });
