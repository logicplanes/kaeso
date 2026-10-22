import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Button, Spinner, Text } from "@logicplanes/kaeso-ui";
import { PublicPage } from "~/shell/public-page";
import { toast } from "~/shell/toast";
import { api } from "~/api/client";
import { usePageTitle } from "~/shell/title";
import { useTokenAction } from "./token-action";
import { EmailCancelPage } from "./email-cancel-page";
import { t } from "~/i18n";

export function EmailConfirmPage() {
  const [cancelling] = useState(() => new URLSearchParams(window.location.hash.slice(1)).has("cancel"));
  return cancelling ? <EmailCancelPage /> : <EmailConfirm />;
}

function EmailConfirm() {
  usePageTitle(t("public.confirmEmail", "Confirm email address"));
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const state = useTokenAction(async (token) => {
    await api.POST("/platform/v1/me/email/confirm", { body: { token } });
    await queryClient.invalidateQueries();
    toast.success(t("public.emailChanged", "Email address changed; your other sessions are signed out"));
    await navigate({ href: "/settings/account/profile", replace: true });
  });
  if (state.kind === "working" || state.kind === "done") {
    return (
      <PublicPage title={t("public.confirmingEmail", "Confirming your email address…")}>
        <div className="flex items-center justify-center gap-8 show-after-delay">
          <Spinner label={t("public.confirming", "Confirming")} />
          <Text tone="secondary">{t("public.oneMoment", "One moment.")}</Text>
        </div>
      </PublicPage>
    );
  }
  if (state.kind === "missing") {
    return (
      <PublicPage
        title={t("public.needsConfirmationLink", "This page needs the confirmation link")}
        description={t("public.needsConfirmationLinkHint", "Open the link from the confirmation email; it ends with #token=…")}
      >
        <Button size="xl" fullWidth href="/settings/account/profile">
          {t("public.goToProfile", "Go to your profile")}
        </Button>
      </PublicPage>
    );
  }
  return (
    <PublicPage
      title={t("public.linkUsed", "This link has expired or was already used.")}
      description={t("public.confirmationLinkOnce", "A confirmation link works once; change the address again for a new one.")}
    >
      <Button variant="primary" size="xl" fullWidth href="/settings/account/profile">
        {t("public.changeAddressAgain", "Change the address again")}
      </Button>
    </PublicPage>
  );
}
