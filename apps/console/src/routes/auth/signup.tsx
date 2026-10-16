import { createFileRoute } from "@tanstack/react-router";
import { SignUpVerifyPage } from "~/features/public/sign-up-page";

export const Route = createFileRoute("/auth/signup")({ component: SignUpVerifyPage });
