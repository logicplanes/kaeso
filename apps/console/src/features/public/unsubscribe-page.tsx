import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Button, Spinner, Text } from "@logicplanes/kaeso-ui";
import { PublicPage } from "~/shell/public-page";
import { api, data } from "~/api/client";
import type { Unsubscribe } from "~/api/generated/types";
import { forgetFragment, takeFragment } from "~/auth/fragment";
import { kindName } from "~/kaeso/notification-kinds";
import { usePageTitle } from "~/shell/title";
import { useSubmit } from "./form";
import { t } from "~/i18n";

const SETTINGS = "/settings/account/notifications";

export function UnsubscribePage() {
  usePageTitle(t("public.unsubscribe", "Unsubscribe"));
  const [token] = useState(() => takeFragment("token"));
  if (!token) {
    return (
      <PublicPage
        title={t("public.needsUnsubscribeLink", "This page needs the unsubscribe link")}
        description={t("public.needsUnsubscribeLinkHint", "Open the link at the bottom of a Kaeso email; it ends with #token=…")}
      >
        <Button size="xl" fullWidth href={SETTINGS}>
          {t("public.openNotificationSettings", "Open notification settings")}
        </Button>
      </PublicPage>
    );
  }
  return <Unsubscribing token={token} />;
}

function Unsubscribing({ token }: { token: string }) {
  const lookup = useQuery({
    queryKey: ["unsubscribe", token],
    retry: false,
    gcTime: 0,
    queryFn: async (): Promise<Unsubscribe> => data(await api.GET("/platform/v1/notifications/unsubscribe", { params: { query: { token } } })),
  });
  const { busy, submit } = useSubmit();
  const [done, setDone] = useState(false);
  const settled = lookup.isError || lookup.data?.delivery === "off" || done;
  useEffect(() => {
    if (settled) forgetFragment(token);
  }, [settled, token]);

  if (lookup.isPending) {
    return (
      <PublicPage title={t("public.openingUnsubscribe", "Opening the link…")}>
        <div className="flex items-center justify-center gap-8 show-after-delay">
          <Spinner label={t("public.lookingUpLink", "Looking up the link")} />
          <Text tone="secondary">{t("public.oneMoment", "One moment.")}</Text>
        </div>
      </PublicPage>
    );
  }
  if (lookup.isError) {
    return (
      <PublicPage
        title={t("public.unsubscribeInvalid", "This unsubscribe link is invalid")}
        description={t("public.unsubscribeInvalidHint", "Sign in and choose your emails under Account › Notifications.")}
      >
        <Button variant="primary" size="xl" fullWidth href={SETTINGS}>
          {t("public.openNotificationSettings", "Open notification settings")}
        </Button>
      </PublicPage>
    );
  }
  const email = lookup.data.email ?? "";
  const kind = kindName(lookup.data.kind, lookup.data.name);
  if (lookup.data.delivery === "off" || done) {
    return (
      <PublicPage
        title={t("public.unsubscribed", "You're unsubscribed")}
        description={t("public.unsubscribedHint", "Kaeso no longer sends “{kind}” to {email}.", { kind, email })}
      >
        <Button size="xl" fullWidth href={SETTINGS}>
          {t("public.chooseOtherEmails", "Choose other emails")}
        </Button>
      </PublicPage>
    );
  }
  return (
    <PublicPage
      title={t("public.unsubscribeFrom", "Unsubscribe from “{kind}”?", { kind })}
      description={t("public.unsubscribeFromHint", "Kaeso stops sending these to {email}; security emails still arrive.", { email })}
    >
      <div className="flex flex-col gap-12">
        <Button
          variant="primary"
          size="xl"
          fullWidth
          loading={busy}
          onClick={() =>
            void submit(async () => {
              data(await api.POST("/platform/v1/notifications/unsubscribe", { params: { query: { token } } }));
              setDone(true);
            })
          }
        >
          {t("public.unsubscribeAction", "Unsubscribe")}
        </Button>
        <Button size="xl" fullWidth href={SETTINGS}>
          {t("public.chooseOtherEmails", "Choose other emails")}
        </Button>
      </div>
    </PublicPage>
  );
}
