import { useEffect, useState } from "react";
import { Text } from "@logicplanes/kaeso-ui";
import { announceConfirmed } from "~/auth/confirm-channel";
import { takeFragment } from "~/auth/fragment";
import { ssoErrorMessage } from "~/auth/sso-errors";
import { t } from "~/i18n";
import { PublicPage } from "~/shell/public-page";
import { usePageTitle } from "~/shell/title";

export function SignedInAgainPage() {
  const [error] = useState(() => takeFragment("sso_error"));
  usePageTitle(error ? t("auth.again.failedTitle", "Not confirmed") : t("auth.again.title", "Signed in again"));
  useEffect(() => {
    announceConfirmed(error);
    window.close();
  }, [error]);
  return (
    <PublicPage title={error ? t("auth.again.failedTitle", "Not confirmed") : t("auth.again.title", "Signed in again")}>
      <Text tone="secondary" className="text-center">
        {error
          ? t("auth.again.failed", "{reason} Close this window and try again in Kaeso.", { reason: ssoErrorMessage(error) })
          : t("auth.again.done", "You can close this window and continue in Kaeso.")}
      </Text>
    </PublicPage>
  );
}
