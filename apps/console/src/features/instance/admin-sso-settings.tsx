import { useQuery } from "@tanstack/react-query";
import { Select, SettingsGroup, SettingsRow, SettingsSection } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { SettingsView } from "~/api/generated/types";
import { instanceSsoOrgsQuery } from "~/api/queries/instance";
import { useApiMutation } from "~/auth/mutation";
import { toast } from "~/shell/toast";
import { t } from "~/i18n";

const NONE = "none";

export function AdminSsoSettings({ settings }: { settings: SettingsView }) {
  const orgs = useQuery(instanceSsoOrgsQuery);
  const current = settings.admin_sso ?? null;
  const save = useApiMutation({
    mutationFn: async (org: string | null) => data(await api.PUT("/platform/v1/instance/settings/admin-sso", { body: { org } })),
    invalidates: [["instance", "settings"], ["session"]],
    onSuccess: (s) =>
      toast.success(
        s.admin_sso
          ? t("instance.adminSso.saved", "{org} can now sign instance admins in", { org: s.admin_sso.name })
          : t("instance.adminSso.cleared", "No organization signs instance admins in"),
      ),
  });
  const listed = orgs.data ?? [];
  const options = [
    { value: NONE, label: t("instance.adminSso.none", "None") },
    ...listed.map((o) => ({ value: o.id, label: o.name })),
    ...(current && !listed.some((o) => o.id === current.org) ? [{ value: current.org, label: current.name }] : []),
  ];
  const description = !current
    ? t("instance.adminSso.noneHint", "Instance admins sign in another way to manage it.")
    : current.connected
      ? t("instance.adminSso.onHint", "Applies to admins whose account the organization manages.")
      : t("instance.adminSso.disconnected", "{org} has no SSO connection, so this has no effect.", { org: current.name });
  return (
    <SettingsSection
      title={t("instance.adminSso.title", "Instance admins")}
      description={t("instance.adminSso.sectionHint", "Changing it needs a recent sign-in with a password or an email link.")}
    >
      <SettingsGroup>
        <SettingsRow
          title={t("instance.adminSso.label", "Single sign-on for instance admins")}
          description={description}
          control={
            <div className="w-240">
              <Select
                aria-label={t("instance.adminSso.label", "Single sign-on for instance admins")}
                size="sm"
                fullWidth
                value={current?.org ?? NONE}
                disabled={save.isPending || orgs.isPending}
                onValueChange={(v: string) => v !== (current?.org ?? NONE) && save.mutate(v === NONE ? null : v)}
                options={options}
              />
            </div>
          }
        />
      </SettingsGroup>
    </SettingsSection>
  );
}
