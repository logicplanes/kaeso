import { SettingsGroup, SettingsRow, Section, Switch } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { SettingsView } from "~/api/generated/types";
import { useApiMutation } from "~/auth/mutation";
import { toast } from "~/shell/toast";
import { t } from "~/i18n";

type Change = { shared_links?: boolean; share_mark?: boolean };

export function SharedLinksSettings({ settings }: { settings: SettingsView }) {
  const save = useApiMutation({
    mutationFn: async (body: Change) => data(await api.PATCH("/platform/v1/instance/settings", { body })),
    invalidates: [["instance"]],
    onSuccess: (_s, body) =>
      toast.success(
        body.shared_links === undefined
          ? t("instance.shareMarkSaved", "Shared pages updated")
          : body.shared_links
            ? t("instance.sharedLinksOn", "Organizations can share links")
            : t("instance.sharedLinksOff", "Shared links are off"),
      ),
  });
  return (
    <Section
      spacing="settings"
      title={t("instance.sharedLinks", "Shared links")}
      description={t("instance.sharedLinksHint", "Read-only links to usage that organizations make for people without an account.")}
    >
      <SettingsGroup>
        <SettingsRow
          title={t("instance.allowSharedLinks", "Allow shared links")}
          description={t("instance.allowSharedLinksHint", "Off, every link stops working and none can be made.")}
          control={
            <Switch
              aria-label={t("instance.allowSharedLinks", "Allow shared links")}
              checked={settings.shared_links}
              disabled={save.isPending}
              onCheckedChange={(on) => save.mutate({ shared_links: on })}
            />
          }
        />
        <SettingsRow
          title={t("instance.shareMark", "Made with Kaeso")}
          description={t("instance.shareMarkHint", "A small mark at the foot of every shared page.")}
          control={
            <Switch
              aria-label={t("instance.shareMark", "Made with Kaeso")}
              checked={settings.share_mark}
              disabled={save.isPending}
              onCheckedChange={(on) => save.mutate({ share_mark: on })}
            />
          }
        />
      </SettingsGroup>
    </Section>
  );
}
