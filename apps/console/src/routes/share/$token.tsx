import { createFileRoute } from "@tanstack/react-router";
import { SharedPage } from "~/features/share/shared-page";

export const Route = createFileRoute("/share/$token")({ component: Shared });

function Shared() {
  return <SharedPage token={Route.useParams().token} />;
}
