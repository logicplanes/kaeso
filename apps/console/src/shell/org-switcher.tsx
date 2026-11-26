import { useEffect } from "react";
import { Avatar, Sidebar, Text } from "@logicplanes/kaeso-ui";
import { useSession } from "~/auth/session";
import { locale, t } from "~/i18n";
import { dateFormatter } from "@logicplanes/kaeso-ui/format";
import { inZone, yearOf } from "~/format/zone";
import { preloadPictures } from "./pictures";
import { OpensWhenReady, useSidebarMenus } from "./sidebar-menus-load";
import { openCreateOrg } from "./create-org-host";

export function deletionCaption(iso: string | null | undefined, now = new Date()): string {
  if (!iso) return t("shell.deletion.scheduled", "Scheduled for deletion");
  const day = new Date(iso);
  const date = dateFormatter({ day: "numeric", month: "short", ...inZone(), ...(yearOf(day) === yearOf(now) ? {} : { year: "numeric" }) }, locale())(day);
  return t("shell.deletion.on", "Deletion on {day}", { day: date });
}

export function OrgSwitcher({ org }: { org: string }) {
  const session = useSession();
  const current = session.memberships.find((m) => m.org_id === org);
  const name = current?.org_name ?? t("common.organization", "Organization");
  const caption = current?.org_state === "scheduled_for_deletion" ? deletionCaption(current.org_deletion_due_at) : null;
  const menus = useSidebarMenus();
  useEffect(() => preloadPictures(session.memberships.map((m) => m.org_avatar_url)), [session.memberships]);

  return (
    <OpensWhenReady ready={!!menus}>
      <Sidebar.Switcher
        side="bottom"
        avatar={<Avatar name={name} {...(current?.org_avatar_url ? { src: current.org_avatar_url } : {})} kind="org" size={20} decorative />}
        label={name}
        {...(caption ? { caption, captionTone: "danger" as const } : {})}
        aria-label={t("shell.orgSwitcher.description", "Switch or open settings")}
      >
        {menus && <menus.OrgMenu org={org} onCreate={openCreateOrg} />}
      </Sidebar.Switcher>
    </OpensWhenReady>
  );
}

export function AccountSwitcher() {
  const session = useSession();
  const picture = session.user.avatar_url ? { src: session.user.avatar_url } : {};
  const menus = useSidebarMenus();
  useEffect(() => preloadPictures([session.user.avatar_url]), [session.user.avatar_url]);

  return (
    <OpensWhenReady ready={!!menus}>
      <Sidebar.Switcher
        avatar={<Avatar name={session.user.name} {...picture} size={24} decorative />}
        label={session.user.name}
        aria-label={t("shell.orgSwitcher.personDescription", "Account and sign out")}
        header={
          <div className="flex min-w-0 items-center gap-10">
            <Avatar name={session.user.name} {...picture} size={32} decorative />
            <div className="flex min-w-0 flex-col">
              <Text weight="medium" truncate>
                {session.user.name}
              </Text>
              <Text variant="small" tone="secondary" truncate>
                {session.user.email}
              </Text>
            </div>
          </div>
        }
      >
        {menus && <menus.AccountMenu onCreate={openCreateOrg} />}
      </Sidebar.Switcher>
    </OpensWhenReady>
  );
}
