import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AlertDialog,
  Button,
  Checkbox,
  CodeBlock,
  CopyButton,
  Dialog,
  DialogScope,
  Field,
  Code,
  Link,
  OTPInput,
  QRCode,
  SensitiveInput,
  SettingsGroup,
  SettingsRow,
  SettingsSection,
  SkeletonLine,
  Text,
} from "@logicplanes/kaeso-ui";
import { toast } from "~/shell/toast";
import { api, data } from "~/api/client";
import { meQuery } from "~/api/queries/me";
import { useApiMutation } from "~/auth/mutation";
import { LoadFailed } from "~/features/org/load-failed";
import { fieldError } from "~/api/field-error";
import { passwordHint, passwordOk } from "~/features/public/form";
import { SettingsLoading, SettingsPage } from "~/shell/settings-page";
import { t } from "~/i18n";

export function SecurityPage() {
  const me = useQuery(meQuery);
  const m = me.data;
  return (
    <SettingsPage title={t("account.security", "Security")} context={t("account.title", "Account")}>
      {me.isError ? (
        <LoadFailed title={t("account.securityFailed", "Your security settings couldn't load")} />
      ) : !m ? (
        <SettingsLoading rows={2} />
      ) : (
        <>
          <PasswordSection hasPassword={m.has_password} />
          <TwoFactorSection enabled={m.totp_enabled} codesLeft={m.recovery_codes_left} />
        </>
      )}
    </SettingsPage>
  );
}

function PasswordSection({ hasPassword }: { hasPassword: boolean }) {
  const [open, setOpen] = useState(false);
  return (
    <SettingsSection title={t("account.password", "Password")}>
      <SettingsGroup>
        <SettingsRow
          title={t("account.password", "Password")}
          description={
            hasPassword
              ? t("account.passwordHint", "Changing it signs out your other sessions.")
              : t("account.passwordNotSet", "Not set; you sign in with a link or SSO.")
          }
          control={
            <Button size="sm" onClick={() => setOpen(true)}>
              {hasPassword ? t("account.changePassword", "Change password") : t("account.setPassword", "Set password")}
            </Button>
          }
        />
      </SettingsGroup>
      <DialogScope open={open}>
        <PasswordDialog hasPassword={hasPassword} open={open} onOpenChange={setOpen} />
      </DialogScope>
    </SettingsSection>
  );
}

function PasswordDialog({ hasPassword, open, onOpenChange: setOpen }: { hasPassword: boolean; open: boolean; onOpenChange: (open: boolean) => void }) {
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const save = useApiMutation({
    mutationFn: async () =>
      data(await api.POST("/platform/v1/me/password", { body: { new_password: next, ...(hasPassword ? { current_password: current } : {}) } })),
    invalidates: [["me"], ["me", "sessions"]],
    onSuccess: () => {
      setOpen(false);
      toast.success(
        hasPassword
          ? t("account.passwordChanged", "Password changed; other sessions signed out")
          : t("account.passwordSet", "Password set; other sessions signed out"),
      );
    },
  });
  const ready = passwordOk(next) && (!hasPassword || current.length > 0);
  return (
    <Dialog
      open={open}
      onOpenChange={setOpen}
      size="sm"
      title={hasPassword ? t("account.changePassword", "Change password") : t("account.setAPassword", "Set a password")}
      onSubmit={() => ready && save.mutate()}
      footer={
        <>
          <Button onClick={() => setOpen(false)}>{t("common.cancel", "Cancel")}</Button>
          <Button variant="primary" disabled={!ready} loading={save.isPending} onClick={() => save.mutate()}>
            {hasPassword ? t("account.changePassword", "Change password") : t("account.setPassword", "Set password")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-16">
        {hasPassword && (
          <SensitiveInput
            label={t("account.currentPassword", "Current password")}
            autoComplete="current-password"
            value={current}
            onChange={(e) => setCurrent(e.target.value)}
            autoFocus
            error={fieldError(save.error, "current_password")}
          />
        )}
        <SensitiveInput
          label={t("account.newPassword", "New password")}
          autoComplete="new-password"
          description={passwordHint(next)}
          value={next}
          onChange={(e) => setNext(e.target.value)}
          error={fieldError(save.error, "new_password")}
        />
      </div>
    </Dialog>
  );
}

function TwoFactorSection({ enabled, codesLeft }: { enabled: boolean; codesLeft: number }) {
  const queryClient = useQueryClient();
  const [twoFactor, setTwoFactor] = useState<{ open: boolean; codes: string[] | null }>({ open: false, codes: null });
  const [turningOff, setTurningOff] = useState(false);
  const [regenerating, setRegenerating] = useState(false);
  const off = useApiMutation({
    mutationFn: async () => {
      await api.DELETE("/platform/v1/me/totp");
    },
    invalidates: [["me"], ["session"]],
    onSuccess: () => {
      setTurningOff(false);
      toast(t("account.twoFactorTurnedOff", "Two-factor authentication turned off"));
    },
    onError: () => setTurningOff(false),
  });
  const regenerate = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/me/recovery-codes")),
    invalidates: [["me"]],
    onSuccess: (r) => {
      setRegenerating(false);
      setTwoFactor({ open: true, codes: r.recovery_codes });
    },
  });
  return (
    <SettingsSection
      title={t("account.twoFactor", "Two-factor authentication")}
      description={t("account.twoFactorHint", "An authenticator code at every password sign-in; sign-in links need none.")}
    >
      <SettingsGroup>
        {enabled ? (
          <>
            <SettingsRow
              title={t("account.authenticatorApp", "Authenticator app")}
              description={t("account.authenticatorOn", "On, so organizations that require it accept your sign-ins.")}
              control={
                <Button size="sm" variant="danger-secondary" onClick={() => setTurningOff(true)}>
                  {t("account.turnOff", "Turn off")}
                </Button>
              }
            />
            <SettingsRow
              title={t("account.recoveryCodes", "Recovery codes")}
              description={t("account.recoveryCodesLeft", "{count} of 10 left; each works for one sign-in.", { count: codesLeft })}
              control={
                <Button size="sm" onClick={() => setRegenerating(true)}>
                  {t("account.regenerate", "Regenerate")}
                </Button>
              }
            />
          </>
        ) : (
          <SettingsRow
            title={t("account.authenticatorApp", "Authenticator app")}
            description={t("account.authenticatorOff", "Off; password sign-ins need only the password.")}
            control={
              <Button size="sm" variant="primary" onClick={() => setTwoFactor({ open: true, codes: null })}>
                {t("account.setUp", "Set up")}
              </Button>
            }
          />
        )}
      </SettingsGroup>
      <DialogScope open={twoFactor.open}>
        <TwoFactorDialog
          open={twoFactor.open}
          initialCodes={twoFactor.codes}
          onClose={() => setTwoFactor((d) => ({ ...d, open: false }))}
          onEnabled={() => {
            void queryClient.invalidateQueries({ queryKey: ["me"] });
            void queryClient.invalidateQueries({ queryKey: ["session"] });
          }}
        />
      </DialogScope>
      <AlertDialog
        open={turningOff}
        onOpenChange={setTurningOff}
        tone="danger"
        title={t("account.turnOffConfirm", "Turn off two-factor authentication?")}
        description={t("account.turnOffConfirmHint", "Password sign-ins need only the password; organizations that require it close to you.")}
        confirmLabel={t("account.turnOff", "Turn off")}
        loading={off.isPending}
        onConfirm={() => off.mutate()}
      />
      <AlertDialog
        open={regenerating}
        onOpenChange={setRegenerating}
        tone="warning"
        title={t("account.regenerateConfirm", "Regenerate recovery codes?")}
        description={t("account.regenerateConfirmHint", "Your current recovery codes stop working; you get ten new ones to store.")}
        confirmLabel={t("account.regenerate", "Regenerate")}
        loading={regenerate.isPending}
        onConfirm={() => regenerate.mutate()}
      />
    </SettingsSection>
  );
}

function TwoFactorDialog({
  open,
  initialCodes,
  onClose,
  onEnabled,
}: {
  open: boolean;
  initialCodes: string[] | null;
  onClose: () => void;
  onEnabled: () => void;
}) {
  const [codes, setCodes] = useState(initialCodes);
  const start = useQuery({
    queryKey: ["totp-setup"],
    queryFn: async () => data(await api.POST("/platform/v1/me/totp")),
    staleTime: Infinity,
    gcTime: 0,
    retry: false,
    enabled: !codes,
  });
  const [code, setCode] = useState("");
  const confirm = useApiMutation({
    mutationFn: async (c: string) => data(await api.POST("/platform/v1/me/totp/confirm", { body: { code: c } })),
    onSuccess: (r) => {
      setCodes(r.recovery_codes);
      onEnabled();
    },
    onError: () => setCode(""),
  });
  const [stored, setStored] = useState(false);
  const failed = start.isError;
  useEffect(() => {
    if (failed) onClose();
  }, [failed, onClose]);
  if (codes) {
    const text = codes.join("\n");
    const download = () => {
      const href = URL.createObjectURL(new Blob([`${t("account.recoveryFileTitle", "Kaeso recovery codes")}\n\n${text}\n`], { type: "text/plain" }));
      const a = document.createElement("a");
      a.href = href;
      a.download = "kaeso-recovery-codes.txt";
      a.click();
      URL.revokeObjectURL(href);
    };
    return (
      <Dialog
        open={open}
        onOpenChange={() => stored && onClose()}
        size="md"
        title={t("account.storeCodes", "Store your recovery codes")}
        description={t("account.storeCodesHint", "Each code signs you in once without your authenticator; they're shown only now.")}
        hideClose
        footer={
          <Button variant="primary" disabled={!stored} onClick={onClose}>
            {t("common.done", "Done")}
          </Button>
        }
      >
        <div className="flex flex-col gap-16">
          <CodeBlock code={text} title={t("account.recoveryCodes", "Recovery codes")} collapseAfter={0} />
          <div className="flex gap-8">
            <Button size="sm" onClick={download}>
              {t("account.downloadTxt", "Download .txt")}
            </Button>
          </div>
          <Checkbox label={t("account.codesStored", "I stored these codes")} checked={stored} onCheckedChange={setStored} />
        </div>
      </Dialog>
    );
  }
  const s = start.data;
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      size="md"
      title={t("account.setupTitle", "Set up two-factor authentication")}
      description={t("account.setupHint", "Add Kaeso to an authenticator app, then enter the six-digit code it shows.")}
      onSubmit={() => code.length === 6 && confirm.mutate(code)}
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel", "Cancel")}</Button>
          <Button variant="primary" disabled={code.length < 6} loading={confirm.isPending} onClick={() => confirm.mutate(code)}>
            {t("account.turnOn", "Turn on")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-20">
        {s ? (
          <div className="flex flex-col items-start gap-20 sm:flex-row">
            <QRCode value={s.otpauth_uri} label={t("account.qrCode", "QR code to add Kaeso to an authenticator app")} />
            <div className="flex min-w-0 flex-col gap-12">
              <Text variant="small" tone="secondary">
                {t("account.scanHint", "Scan the code with the app, or enter the setup key.")}
              </Text>
              <Field label={t("account.setupKey", "Setup key")}>
                <div className="flex items-center gap-8">
                  <Code>{s.secret}</Code>
                  <CopyButton value={s.secret} label={t("account.copySetupKey", "Copy setup key")} size="xs" />
                </div>
              </Field>
              <Text variant="small" tone="secondary">
                <Link href={s.otpauth_uri}>{t("account.openInApp", "Open in an authenticator app on this device")}</Link>
              </Text>
            </div>
          </div>
        ) : (
          <SkeletonLine lines={6} />
        )}
        <Field label={t("account.appCode", "Code from the app")}>
          <OTPInput aria-label={t("account.appCode", "Code from the app")} value={code} onValueChange={setCode} onComplete={(v) => confirm.mutate(v)} />
        </Field>
      </div>
    </Dialog>
  );
}
