import { useQueryClient } from "@tanstack/react-query";
import { useRouter, type ErrorComponentProps } from "@tanstack/react-router";
import { useEffect } from "react";
import { AppFrame, Button, Empty, PageHeader } from "@logicplanes/kaeso-ui";
import { toast } from "~/shell/toast";
import { isApiError } from "~/api/errors";
import { toastApiError } from "~/api/toast-error";
import { t } from "~/i18n";

export function RouteError({ error, reset }: ErrorComponentProps) {
  const router = useRouter();
  const queryClient = useQueryClient();
  useEffect(() => {
    if (isApiError(error)) {
      if (error.type !== "not_found" && error.type !== "permission_denied") toastApiError(error, { id: `load:${error.type}:${error.message}` });
    } else toast.error(t("shell.routeError.unexpected", "This page hit an unexpected error; reload to try again"), { id: "route-error" });
  }, [error]);
  const retry = async () => {
    await queryClient.invalidateQueries({ predicate: (query) => query.state.status === "error" });
    reset();
    await router.invalidate();
  };
  if (isApiError(error) && error.type === "not_found") {
    return (
      <>
        <PageHeader start={<AppFrame.MenuButton />} title={t("shell.notFound", "Not found")} />
        <AppFrame.Body>
          <Empty
            size="lg"
            title={t("shell.routeError.gone", "This doesn't exist or was removed")}
            description={error.message}
            actions={<Button href="/">{t("shell.routeError.start", "Go to the start page")}</Button>}
          />
        </AppFrame.Body>
      </>
    );
  }
  if (isApiError(error) && error.type === "permission_denied") {
    return (
      <>
        <PageHeader start={<AppFrame.MenuButton />} title={t("shell.routeError.noAccess", "No access")} />
        <AppFrame.Body>
          <Empty size="lg" title={t("shell.routeError.noAccessTitle", "You don't have access to this page")} description={error.message} />
        </AppFrame.Body>
      </>
    );
  }
  return (
    <>
      <PageHeader start={<AppFrame.MenuButton />} title={t("shell.routeError.header", "Couldn't load this page")} />
      <AppFrame.Body>
        <Empty
          size="lg"
          title={t("shell.routeError.title", "This page couldn't load")}
          description={t("shell.routeError.description", "Try again in a moment.")}
          actions={<Button onClick={() => void retry()}>{t("common.tryAgain", "Try again")}</Button>}
        />
      </AppFrame.Body>
    </>
  );
}
