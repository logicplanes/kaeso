import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Button, Spinner, Text } from "@logicplanes/kaeso-ui";
import { PublicPage } from "~/shell/public-page";
import { api } from "~/api/client";
import { usePageTitle } from "~/shell/title";
import { safeNext } from "~/auth/next";
import { useTokenAction } from "./token-action";
import { t } from "~/i18n";

export function MagicLinkPage({ next }: { next?: string | undefined }) {
  usePageTitle(t("public.signingIn", "Signing in"));
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const state = useTokenAction(async (token) => {
    await api.POST("/platform/v1/auth/magic-link/verify", { body: { token } });
    await queryClient.resetQueries();
    await navigate({ to: safeNext(next), replace: true });
  });
  if (state.kind === "working" || state.kind === "done") {
    return (
      <PublicPage title={t("public.signingYouInNow", "Signing you in…")}>
        <div className="flex items-center justify-center gap-8 show-after-delay">
          <Spinner label={t("public.signingYouIn", "Signing you in")} />
          <Text tone="secondary">{t("public.oneMoment", "One moment.")}</Text>
        </div>
      </PublicPage>
    );
  }
  if (state.kind === "missing") {
    return (
      <PublicPage
        title={t("public.needsSignInLink", "This page needs the sign-in link")}
        description={t("public.needsSignInLinkHint", "Open the link from the sign-in email; it ends with #token=…")}
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
      description={t("public.signInLinkOnce", "A sign-in link works once, for 15 minutes.")}
    >
      <Button variant="primary" size="xl" fullWidth href="/sign-in">
        {t("public.requestNewLink", "Request a new link")}
      </Button>
    </PublicPage>
  );
}
