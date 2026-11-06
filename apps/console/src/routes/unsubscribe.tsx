import { createFileRoute } from "@tanstack/react-router";
import { UnsubscribePage } from "~/features/public/unsubscribe-page";

export const Route = createFileRoute("/unsubscribe")({ component: UnsubscribePage });
