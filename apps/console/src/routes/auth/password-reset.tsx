import { createFileRoute } from "@tanstack/react-router";
import { PasswordResetPage } from "~/features/public/password-reset-page";

export const Route = createFileRoute("/auth/password-reset")({ component: PasswordResetPage });
