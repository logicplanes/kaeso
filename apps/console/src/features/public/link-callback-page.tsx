import { useEffect, useRef, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { Button, Spinner } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { CredentialView } from "~/api/generated/types";
import { t } from "~/i18n";
import { takeLink, type PendingLink } from "~/kaeso/linked-billing";
import { PublicPage } from "~/shell/public-page";
import { toast } from "~/shell/toast";
import { usePageTitle } from "~/shell/title";

// The address the provider sent the browser back to carries a one-time code: it is read once and
// leaves the address bar before anything else runs.
function returnedAddress(): string {
  const address = window.location.href;
  window.history.replaceState(window.history.state, "", window.location.pathname);
  return address;
}

async function finish(link: PendingLink, address: string): Promise<CredentialView> {
  const body = { callback_url: address };
  if (link.credential)
    return data(
      await api.POST("/platform/v1/orgs/{org}/credentials/{credential}/oauth/{attempt}", {
        params: { path: { org: link.org, credential: link.credential, attempt: link.attempt } },
        body,
      }),
    ) as CredentialView;
  return data(
    await api.POST("/platform/v1/orgs/{org}/credentials/oauth/{attempt}", { params: { path: { org: link.org, attempt: link.attempt } }, body }),
  ) as CredentialView;
}

export function LinkCallbackPage() {
  usePageTitle(t("public.linking", "Linking your organization"));
  const navigate = useNavigate();
  const [link] = useState(takeLink);
  const [address] = useState(returnedAddress);
  const [failed, setFailed] = useState<string | null>(null);
  const started = useRef(false);
  useEffect(() => {
    if (!link || started.current) return;
    started.current = true;
    finish(link, address).then(
      (key) => {
        toast.success(t("public.linked", "{name} linked", { name: key.name }));
        void navigate({ to: "/orgs/$org/providers", params: { org: link.org }, replace: true });
      },
      // The client turns every failure, a network one included, into an `ApiError` with its message.
      (error: { message: string }) => setFailed(error.message),
    );
  }, [link, address, navigate]);
  if (!link)
    return (
      <PublicPage
        title={t("public.linkUnusable", "This link can't be finished here")}
        description={t("public.linkUnusableHint", "Start linking again under Providers, in the same browser tab.")}
      >
        <Button size="xl" fullWidth href="/">
          {t("public.openKaeso", "Open Kaeso")}
        </Button>
      </PublicPage>
    );
  if (failed)
    return (
      <PublicPage title={t("public.linkFailed", "Linking didn't finish")} description={failed}>
        <Button size="xl" fullWidth href={`/orgs/${link.org}/providers`}>
          {t("public.backToProviders", "Back to Providers")}
        </Button>
      </PublicPage>
    );
  return (
    <PublicPage title={t("public.linking", "Linking your organization")}>
      <div className="flex justify-center show-after-delay">
        <Spinner label={t("public.finishingLink", "Finishing the link")} />
      </div>
    </PublicPage>
  );
}
