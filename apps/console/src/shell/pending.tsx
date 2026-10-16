import { Spinner } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";
import { PublicPage } from "./public-page";

export function SessionPending() {
  return (
    <PublicPage title={t("shell.opening", "Opening Kaeso…")}>
      <div className="flex justify-center">
        <Spinner size="sm" label={t("shell.loadingSession", "Loading your session")} />
      </div>
    </PublicPage>
  );
}
