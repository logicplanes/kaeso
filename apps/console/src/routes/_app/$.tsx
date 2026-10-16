import { createFileRoute } from "@tanstack/react-router";
import { NotFound } from "~/shell/not-found";

export const Route = createFileRoute("/_app/$")({
  component: NotFound,
});
