import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Button,
  Code,
  Dialog,
  DialogScope,
  Input,
  NumberInput,
  SettingsGroup,
  SettingsRow,
  SettingsSection,
  StatusDot,
  Switch,
  Text,
} from "@logicplanes/kaeso-ui";
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

export function InstanceSettingsPage() {
  const settings = useQuery(instanceSettingsQuery);
  const s = settings.data;
  return (
    <InstancePage title={t("common.settings", "Settings")} description={t("instance.settingsHint", "How the install works for everyone on it.")}>
      {settings.isError ? (
        <LoadFailed title={t("instance.settingsFailed", "The instance settings couldn't load")} />
      ) : !s ? (
        <SettingsLoading rows={4} />
      ) : (
        <>
          <AccountsSettings settings={s} />
          <OrgDeletion key={s.org_deletion_grace_days} settings={s} />
          <Retention
            key={`${s.metadata_retention_days}-${s.content_retention_days}-${s.max_metadata_retention_days}-${s.max_content_retention_days}`}
            settings={s}
          />
          <SignIn settings={s} />
          <Mail settings={s} />
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

function Retention({ settings }: { settings: SettingsView }) {
  const [metadata, setMetadata] = useState(String(settings.metadata_retention_days));
  const [content, setContent] = useState(String(settings.content_retention_days));
  const [maxMetadata, setMaxMetadata] = useState(String(settings.max_metadata_retention_days));
  const [maxContent, setMaxContent] = useState(String(settings.max_content_retention_days));
  const save = useApiMutation({
    mutationFn: async () =>
      data(
        await api.PATCH("/platform/v1/instance/settings", {
          body: {
            metadata_retention_days: Number(metadata),
            content_retention_days: Number(content),
            max_metadata_retention_days: Number(maxMetadata),
            max_content_retention_days: Number(maxContent),
          },
        }),
      ),
    invalidates: [["instance", "settings"]],
    onSuccess: () => toast.success(t("instance.retentionSaved", "Retention saved")),
  });
  const [m, c, mm, mc] = [metadata, content, maxMetadata, maxContent].map(Number) as [number, number, number, number];
  const whole = (n: number) => Number.isInteger(n) && n >= 1;
  const changed =
    m !== settings.metadata_retention_days ||
    c !== settings.content_retention_days ||
    mm !== settings.max_metadata_retention_days ||
    mc !== settings.max_content_retention_days;
  const reason = ![m, c, mm, mc].every(whole)
    ? t("instance.wholeDays", "Enter whole days, at least 1")
    : m > mm
      ? t("instance.metadataAboveMax", "The default metadata retention is above its maximum")
      : c > mc
        ? t("instance.contentAboveMax", "The default content retention is above its maximum")
        : c > m || mc > mm
          ? t("instance.contentLongerThanMetadata", "Content can't be kept longer than metadata")
          : !changed
            ? t("instance.noChange", "No change to save")
            : undefined;
  const field = (label: string, value: string, set: (v: string) => void) => (
    <div className="w-160">
      <NumberInput aria-label={label} size="sm" value={value} onValueChange={set} min="1" step="1" unit={t("instance.daysUnit", "days")} />
    </div>
  );
  return (
    <SettingsSection
      title={t("instance.retention", "Request log retention")}
      description={t("instance.retentionHint", "Defaults for new organizations, and the most any organization may choose.")}
    >
      <SettingsGroup>
        <SettingsRow
          title={t("instance.metadataDefault", "Metadata, by default")}
          description={t("instance.metadataHint", "Timing, tokens, cost and errors.")}
          control={field(t("instance.metadataDefaultLabel", "Default metadata retention"), metadata, setMetadata)}
        />
        <SettingsRow
          title={t("instance.contentDefault", "Content, by default")}
          description={t("instance.contentHint", "Request and reply bodies.")}
          control={field(t("instance.contentDefaultLabel", "Default content retention"), content, setContent)}
        />
        <SettingsRow
          title={t("instance.metadataMax", "Metadata, at most")}
          control={field(t("instance.metadataMaxLabel", "Maximum metadata retention"), maxMetadata, setMaxMetadata)}
        />
        <SettingsRow
          title={t("instance.contentMax", "Content, at most")}
          control={field(t("instance.contentMaxLabel", "Maximum content retention"), maxContent, setMaxContent)}
        />
      </SettingsGroup>
      <div className="mt-12 flex justify-end">
        <Button
          size="sm"
          variant="primary"
          disabled={!!reason}
          {...(reason ? { disabledReason: reason } : {})}
          loading={save.isPending}
          onClick={() => save.mutate()}
        >
          {t("instance.saveRetention", "Save retention")}
        </Button>
      </div>
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

function Mail({ settings }: { settings: SettingsView }) {
  const [open, setOpen] = useState(false);
  const mail = settings.mail;
  return (
    <SettingsSection
      title={t("instance.mail", "Mail")}
      description={t("instance.setInConfig", "Set in the gateway's configuration.")}
      actions={
        <Button size="sm" onClick={() => setOpen(true)}>
          {t("instance.sendTestEmail", "Send test email")}
        </Button>
      }
    >
      <SettingsGroup>
        <SettingsRow title={t("instance.transport", "Transport")} control={<Code>{mail.transport}</Code>} />
        {mail.host && <SettingsRow title={t("instance.host", "Host")} control={<Code>{`${mail.host}:${mail.port}`}</Code>} />}
        <SettingsRow title={t("instance.from", "From")} control={<Text tone="secondary">{mail.from}</Text>} />
      </SettingsGroup>
      <DialogScope open={open}>
        <TestEmailDialog open={open} onOpenChange={setOpen} />
      </DialogScope>
    </SettingsSection>
  );
}

function TestEmailDialog({ open, onOpenChange: setOpen }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const [to, setTo] = useState("");
  const send = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/instance/settings/test-email", { body: to.trim() ? { to: to.trim() } : {} })),
    onSuccess: (r) => {
      setOpen(false);
      toast.success(t("instance.testEmailSent", "Test email sent to {email}", { email: r.to }));
    },
  });
  const reason =
    to.trim() && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(to.trim()) ? t("instance.emailOrEmpty", "Enter an email address, or leave it empty") : undefined;
  return (
    <Dialog
      open={open}
      onOpenChange={setOpen}
      size="sm"
      title={t("instance.testEmailTitle", "Send a test email")}
      description={t("instance.testEmailHint", "Checks that the gateway can send mail.")}
      onSubmit={() => !reason && send.mutate()}
      footer={
        <>
          <Button onClick={() => setOpen(false)}>{t("common.cancel", "Cancel")}</Button>
          <Button variant="primary" disabled={!!reason} {...(reason ? { disabledReason: reason } : {})} loading={send.isPending} onClick={() => send.mutate()}>
            {t("instance.sendTestEmail", "Send test email")}
          </Button>
        </>
      }
    >
      <Input
        label={t("instance.recipient", "Recipient")}
        optional
        description={t("instance.recipientHint", "Leave it empty to send it to yourself.")}
        type="email"
        autoComplete="off"
        value={to}
        onChange={(e) => setTo(e.target.value)}
        autoFocus
      />
    </Dialog>
  );
}
