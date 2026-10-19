import { Button, Spinner, Text } from "@logicplanes/kaeso-ui";
import { PublicPage } from "~/shell/public-page";
import { api } from "~/api/client";
import { usePageTitle } from "~/shell/title";
import { useTokenAction } from "./token-action";
import { t } from "~/i18n";

export function EmailCancelPage() {
  usePageTitle(t("public.cancelEmailChange", "Cancel email change"));
  const state = useTokenAction(async (token) => {
    await api.POST("/platform/v1/me/email/cancel", { body: { token } });
  });
  if (state.kind === "working") {
    return (
      <PublicPage title={t("public.cancellingEmailChange", "Cancelling the email change…")}>
        <div className="flex items-center justify-center gap-8 show-after-delay">
          <Spinner label={t("public.cancelling", "Cancelling")} />
          <Text tone="secondary">{t("public.oneMoment", "One moment.")}</Text>
        </div>
      </PublicPage>
    );
  }
  if (state.kind === "done") {
    return (
      <PublicPage
        title={t("public.emailChangeCancelled", "The email change is cancelled")}
        description={t("public.emailChangeCancelledHint", "Your account keeps this address; if you didn't ask for the change, reset your password.")}
      >
        <Button variant="primary" size="xl" fullWidth href="/sign-in">
          {t("public.toSignIn", "Go to sign in")}
        </Button>
      </PublicPage>
    );
  }
  if (state.kind === "missing") {
    return (
      <PublicPage
        title={t("public.needsCancelLink", "This page needs the cancel link")}
        description={t("public.needsCancelLinkHint", "Open the link from the email about the change; it ends with #token=…")}
      >
        <Button size="xl" fullWidth href="/sign-in">
          {t("public.backToSignIn", "Back to sign in")}
        </Button>
      </PublicPage>
    );
  }
  return (
    <PublicPage
      title={t("public.linkUsed", "This link has expired or was already used.")}
      description={t("public.cancelLinkOnce", "The change may be done or cancelled already; check your account's address after signing in.")}
    >
      <Button variant="primary" size="xl" fullWidth href="/sign-in">
        {t("public.toSignIn", "Go to sign in")}
      </Button>
    </PublicPage>
  );
}
