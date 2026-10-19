import type { ReactNode } from "react";
import { Outlet } from "@tanstack/react-router";
import { useQueryClient } from "@tanstack/react-query";
import { Button, Empty } from "@logicplanes/kaeso-ui";
import { requestRecentSignIn } from "~/auth/recent-sign-in-store";
import { sessionQuery, useSession } from "~/auth/session";
import { SettingsPage } from "~/shell/settings-page";
import { t } from "~/i18n";

export function InstanceGate() {
  const session = useSession();
  const queryClient = useQueryClient();
  if (session.user.is_instance_admin && !session.can_administer_instance) {
    const signIn = async () => {
      if (await requestRecentSignIn("without-sso")) await queryClient.invalidateQueries({ queryKey: sessionQuery.queryKey });
    };
    return (
      <SettingsPage title={t("instance.title", "Instance")} context={t("instance.title", "Instance")}>
        <Empty
          title={t("instance.signInAnotherWay", "Sign in another way to manage the instance")}
          description={t("instance.signInAnotherWayHint", "Your SSO sign-in can't manage the instance; sign in with your password or an email link.")}
          actions={
            <Button variant="primary" onClick={() => void signIn()}>
              {t("auth.otherSignIn.action", "Sign in another way")}
            </Button>
          }
        />
      </SettingsPage>
    );
  }
  if (!session.can_administer_instance) {
    return (
      <SettingsPage title={t("instance.title", "Instance")} context={t("instance.title", "Instance")}>
        <Empty
          title={t("instance.adminsOnly", "For instance admins")}
          description={t("instance.adminsOnlyHint", "These pages manage the whole install; an instance admin can make you one.")}
        />
      </SettingsPage>
    );
  }
  return <Outlet />;
}

export function InstancePage({ title, description, actions, children }: { title: string; description?: ReactNode; actions?: ReactNode; children: ReactNode }) {
  return (
    <SettingsPage title={title} context={t("instance.title", "Instance")} {...(description ? { description } : {})} {...(actions ? { actions } : {})}>
      {children}
    </SettingsPage>
  );
}
