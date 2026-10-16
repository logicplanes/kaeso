import type { ReactNode } from "react";
import { Outlet } from "@tanstack/react-router";
import { Empty } from "@logicplanes/kaeso-ui";
import { useSession } from "~/auth/session";
import { SettingsPage } from "~/shell/settings-page";
import { t } from "~/i18n";

export function InstanceGate() {
  const session = useSession();
  if (!session.user.is_instance_admin) {
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
