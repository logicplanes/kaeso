import { createFileRoute } from "@tanstack/react-router";
import { SetupPage } from "~/features/public/setup-page";
import { signInMethodsQuery } from "~/auth/sign-in-api";

export const Route = createFileRoute("/setup")({
  loader: ({ context }) => context.queryClient.prefetchQuery(signInMethodsQuery),
  component: SetupPage,
});
