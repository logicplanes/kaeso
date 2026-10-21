import { createFileRoute } from "@tanstack/react-router";
import { SignedInAgainPage } from "~/features/public/signed-in-again-page";

export const Route = createFileRoute("/signed-in-again")({ component: SignedInAgainPage });
