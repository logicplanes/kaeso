import { useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { isApiError } from "~/api/errors";
import { sessionQuery } from "~/auth/session";
import { t } from "~/i18n";
import { useWatchConnection } from "./connection-watch";
import { toast } from "./toast";

const ID = "gateway-unreachable";

export function ConnectionToast() {
  const session = useQuery({
    ...sessionQuery,
    refetchInterval: (q) => (q.state.status === "error" ? 5000 : 60_000),
    retry: false,
  });
  const error = session.error;
  const down = isApiError(error) && error.unreachable;
  const failures = session.errorUpdateCount;
  const { refetch } = session;
  useWatchConnection();
  useEffect(() => {
    if (!down) return;
    toast.warning(t("shell.connection.retrying", "Can't reach the gateway; retrying"), {
      id: ID,
      timeout: 0,
      action: { label: t("shell.connection.retryNow", "Retry now"), onClick: () => void refetch() },
    });
  }, [down, failures, refetch]);
  useEffect(() => {
    if (!down) toast.dismiss(ID);
  }, [down]);
  return null;
}
