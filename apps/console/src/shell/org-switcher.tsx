import { Suspense, lazy, useEffect, useState } from "react";
import { Avatar, Sidebar, Text } from "@logicplanes/kaeso-ui";
import { useSession } from "~/auth/session";
import { locale, t } from "~/i18n";
import { dateFormatter } from "@logicplanes/kaeso-ui/format";
import { inZone, yearOf } from "~/format/zone";
import { preloadPictures } from "./pictures";
import { OpensWhenReady, useSidebarMenus } from "./sidebar-menus-load";

export function deletionCaption(iso: string | null | undefined, now = new Date()): string {
  if (!iso) return t("shell.deletion.scheduled", "Scheduled for deletion");
  const day = new Date(iso);
  const date = dateFormatter({ day: "numeric", month: "short", ...inZone(), ...(yearOf(day) === yearOf(now) ? {} : { year: "numeric" }) }, locale())(day);
  return t("shell.deletion.on", "Deletion on {day}", { day: date });
}

const CreateOrgDialog = lazy(() => import("./create-org-dialog"));

export function OrgSwitcher({ org }: { org: string | undefined }) {
  const session = useSession();
  const [creating, setCreating] = useState(false);
  const current = session.memberships.find((m) => m.org_id === org);
  const name = org ? (current?.org_name ?? t("common.organization", "Organization")) : session.user.name;
  const caption = current?.org_state === "scheduled_for_deletion" ? deletionCaption(current.org_deletion_due_at) : null;
  const avatar = org ? (
    <Avatar name={name} {...(current?.org_avatar_url ? { src: current.org_avatar_url } : {})} kind="org" size={24} decorative />
  ) : (
    <Avatar name={name} {...(session.user.avatar_url ? { src: session.user.avatar_url } : {})} size={24} decorative />
  );
  const menus = useSidebarMenus();
  useEffect(() => preloadPictures([session.user.avatar_url]), [session.user.avatar_url]);

  return (
    <>
      <OpensWhenReady ready={!!menus}>
        <Sidebar.Switcher
          avatar={avatar}
          label={name}
          {...(caption ? { caption, captionTone: "danger" as const } : {})}
          aria-label={org ? t("shell.orgSwitcher.description", "Switch or open settings") : t("shell.orgSwitcher.personDescription", "Account and sign out")}
          header={
            <div className="flex min-w-0 items-center gap-10">
              <Avatar name={session.user.name} {...(session.user.avatar_url ? { src: session.user.avatar_url } : {})} size={32} decorative />
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
          {menus && <menus.OrgMenu org={org} onCreate={() => setCreating(true)} />}
        </Sidebar.Switcher>
      </OpensWhenReady>
      {creating && (
        <Suspense fallback={null}>
          <CreateOrgDialog onClose={() => setCreating(false)} />
        </Suspense>
      )}
    </>
  );
}
