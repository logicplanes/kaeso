import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Button, NumberInput, SettingsGroup, SettingsRow, SettingsSection, StatusDot, Switch } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { SettingsView } from "~/api/generated/types";
import { instanceSettingsQuery } from "~/api/queries/instance";
import { useApiMutation } from "~/auth/mutation";
import { toast } from "~/shell/toast";
import { SettingsLoading } from "~/shell/settings-page";
import { LoadFailed } from "~/features/org/load-failed";
import { AccountsSettings } from "./accounts-settings";
import { InstancePage } from "./instance-page";
import { t } from "~/i18n";

export function InstanceGeneralPage() {
  const settings = useQuery(instanceSettingsQuery);
  const s = settings.data;
  return (
    <InstancePage
      title={t("shell.settings.general", "General")}
      description={t("instance.generalHint", "Who can join this install, and what organizations may do.")}
    >
      {settings.isError ? (
        <LoadFailed title={t("instance.settingsFailed", "The instance settings couldn't load")} />
      ) : !s ? (
        <SettingsLoading rows={4} />
      ) : (
        <>
          <AccountsSettings settings={s} />
          <SignIn settings={s} />
          <OrgDeletion key={s.org_deletion_grace_days} settings={s} />
        </>
      )}
    </InstancePage>
  );
}

const GRACE_DAYS = { min: 1, max: 90 };

function OrgDeletion({ settings }: { settings: SettingsView }) {
  const [days, setDays] = useState(String(settings.org_deletion_grace_days));
  const save = useApiMutation({
    mutationFn: async (body: { org_deletion?: boolean; org_deletion_grace_days?: number }) => data(await api.PATCH("/platform/v1/instance/settings", { body })),
    invalidates: [["instance", "settings"], ["session"]],
    onSuccess: (s, body) =>
      toast.success(
        body.org_deletion === undefined
          ? t("instance.deletionsWait", "Deletions wait {count, plural, one {# day} other {# days}}", { count: s.org_deletion_grace_days })
          : s.org_deletion
            ? t("instance.orgsDeletable", "Organizations can be deleted")
            : t("instance.orgsNotDeletable", "Organizations can't be deleted"),
      ),
  });
  const n = Number(days);
  const reason =
    !Number.isInteger(n) || n < GRACE_DAYS.min || n > GRACE_DAYS.max
      ? t("instance.enterGraceDays", "Enter {min} to {max} days", GRACE_DAYS)
      : n === settings.org_deletion_grace_days
        ? t("instance.noChange", "No change to save")
        : undefined;
  return (
    <SettingsSection
      title={t("instance.deletingOrgs", "Deleting organizations")}
      description={t("instance.deletingOrgsHint", "Owners delete their organization in its settings.")}
    >
      <SettingsGroup>
        <SettingsRow
          title={t("instance.orgsDeletable", "Organizations can be deleted")}
          description={t("instance.orgsDeletableHint", "When off, no organization can be deleted, not even here.")}
          control={
            <Switch
              aria-label={t("instance.orgsDeletable", "Organizations can be deleted")}
              checked={settings.org_deletion}
              loading={save.isPending && save.variables?.org_deletion !== undefined}
              onCheckedChange={(on) => save.mutate({ org_deletion: on })}
            />
          }
        />
        <SettingsRow
          title={t("instance.gracePeriod", "Grace period")}
          description={t("instance.gracePeriodHint", "Read-only and restorable until then.")}
          control={
            <div className="flex items-center gap-8">
              <div className="w-160">
                <NumberInput
                  aria-label={t("instance.gracePeriod", "Grace period")}
                  size="sm"
                  value={days}
                  onValueChange={setDays}
                  min={String(GRACE_DAYS.min)}
                  max={String(GRACE_DAYS.max)}
                  step="1"
                  unit={t("instance.daysUnit", "days")}
                />
              </div>
              <Button
                size="sm"
                disabled={!!reason}
                {...(reason ? { disabledReason: reason } : {})}
                loading={save.isPending && save.variables?.org_deletion_grace_days !== undefined}
                onClick={() => save.mutate({ org_deletion_grace_days: n })}
              >
                {t("common.save", "Save")}
              </Button>
            </div>
          }
        />
      </SettingsGroup>
    </SettingsSection>
  );
}

function SignIn({ settings }: { settings: SettingsView }) {
  const methods: [string, boolean][] = [
    [t("instance.password", "Password"), settings.sign_in.password],
    [t("instance.signInLink", "Sign-in link"), settings.sign_in.magic_link],
    [t("instance.sso", "Single sign-on"), settings.sign_in.sso],
  ];
  return (
    <SettingsSection title={t("instance.signInMethods", "Sign-in methods")} description={t("instance.setInConfig", "Set in the gateway's configuration.")}>
      <SettingsGroup>
        {methods.map(([label, on]) => (
          <SettingsRow
            key={label}
            title={label}
            control={<StatusDot tone={on ? "success" : "neutral"}>{on ? t("common.on", "On") : t("common.off", "Off")}</StatusDot>}
          />
        ))}
      </SettingsGroup>
    </SettingsSection>
  );
}
