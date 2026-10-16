import { createFileRoute } from "@tanstack/react-router";
import { SignUpPage } from "~/features/public/sign-up-page";

export const Route = createFileRoute("/sign-up")({ component: SignUpPage });
