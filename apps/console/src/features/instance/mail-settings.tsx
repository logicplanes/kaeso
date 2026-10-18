import { useState } from "react";
import { Button, Code, Dialog, DialogScope, Input, SettingsGroup, SettingsRow, SettingsSection, Text } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { SettingsView } from "~/api/generated/types";
import { useApiMutation } from "~/auth/mutation";
import { toast } from "~/shell/toast";
import { t } from "~/i18n";

export function Mail({ settings }: { settings: SettingsView }) {
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
      busy={send.isPending}
      open={open}
      onOpenChange={setOpen}
      size="sm"
      title={t("instance.testEmailTitle", "Send a test email")}
      description={t("instance.testEmailHint", "Checks that the gateway can send mail.")}
      onSubmit={() => !reason && send.mutate()}
      footer={
        <>
          <Dialog.Close>
            <Button>{t("common.cancel", "Cancel")}</Button>
          </Dialog.Close>
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
