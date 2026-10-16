import { useEffect, useEffectEvent } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useRouter, type ErrorComponentProps } from "@tanstack/react-router";
import { Button } from "@logicplanes/kaeso-ui";
import { PublicPage } from "~/shell/public-page";
import { isApiError } from "~/api/errors";
import { sessionQuery } from "~/auth/session";
import { t } from "~/i18n";
import { toastApiError } from "~/api/toast-error";
import { useWatchConnection } from "./connection-watch";
import { toast } from "./toast";

export function GatewayUnreachable({ error, reset }: ErrorComponentProps) {
  const router = useRouter();
  const queryClient = useQueryClient();
  useWatchConnection();
  const retry = async () => {
    try {
      await queryClient.fetchQuery({ ...sessionQuery, retry: false });
    } catch {
      return;
    }
    reset();
    await router.invalidate();
  };
  useEffect(() => {
    const timer = window.setInterval(() => void retry(), 5000);
    return () => window.clearInterval(timer);
  });
  const id = isApiError(error) ? `load:${error.type}:${error.message}` : "session-load";
  const show = useEffectEvent(() => {
    if (isApiError(error) && !error.unreachable) toastApiError(error, { id });
    else
      toast.warning(t("shell.connection.retrying", "Can't reach the gateway; retrying"), {
        id,
        timeout: 0,
        action: { label: t("shell.connection.retryNow", "Retry now"), onClick: () => void retry() },
      });
  });
  useEffect(() => {
    show();
    return () => toast.dismiss(id);
  }, [id]);
  return (
    <PublicPage
      title={t("shell.unreachable.title", "Kaeso isn't answering")}
      description={t("shell.unreachable.description", "The console reaches the gateway on this domain; check that it's running.")}
    >
      <Button size="xl" fullWidth onClick={() => void retry()}>
        {t("shell.connection.retryNow", "Retry now")}
      </Button>
    </PublicPage>
  );
}
