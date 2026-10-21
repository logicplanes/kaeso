import { useCallback, useEffect, useRef, useState } from "react";
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
import { meQuery, ssoLinksQuery } from "~/api/queries/me";
import { useApiMutation } from "~/auth/mutation";
import { isApiError } from "~/api/errors";
import { requestRecentSignIn } from "~/auth/recent-sign-in-store";
import { NO_SECOND_FACTOR, SecondFactorField, secondFactorBody, secondFactorReady, type SecondFactorValue } from "./second-factor";
import { LoadFailed } from "~/features/org/load-failed";
import { fieldError } from "~/api/field-error";
import { passwordHint, passwordOk } from "~/features/public/form";
import { SettingsLoading, SettingsPage } from "~/shell/settings-page";
import { t } from "~/i18n";

export function SecurityPage() {
  const me = useQuery(meQuery);
  const m = me.data;
  return (
    <SettingsPage
      title={t("account.security", "Security")}
      context={t("account.title", "Account")}
      description={t("account.securityHint", "How you sign in: your password, a second factor and single sign-on.")}
    >
      {me.isError ? (
        <LoadFailed title={t("account.securityFailed", "Your security settings couldn't load")} />
      ) : !m ? (
        <SettingsLoading rows={2} />
      ) : (
        <>
          <PasswordSection hasPassword={m.has_password} email={m.email} />
          <TwoFactorSection enabled={m.totp_enabled} codesLeft={m.recovery_codes_left} />
          <SingleSignOnSection />
        </>
      )}
    </SettingsPage>
  );
}

function PasswordSection({ hasPassword, email }: { hasPassword: boolean; email: string | null | undefined }) {
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
        <PasswordDialog hasPassword={hasPassword} email={email} open={open} onOpenChange={setOpen} />
      </DialogScope>
    </SettingsSection>
  );
}

function PasswordDialog({
  hasPassword,
  email,
  open,
  onOpenChange: setOpen,
}: {
  hasPassword: boolean;
  email: string | null | undefined;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
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
      busy={save.isPending}
      open={open}
      onOpenChange={setOpen}
      size="sm"
      title={hasPassword ? t("account.changePassword", "Change password") : t("account.setAPassword", "Set a password")}
      onSubmit={() => ready && save.mutate()}
      footer={
        <>
          <Dialog.Close>
            <Button>{t("common.cancel", "Cancel")}</Button>
          </Dialog.Close>
          <Button variant="primary" disabled={!ready} loading={save.isPending} onClick={() => save.mutate()}>
            {hasPassword ? t("account.changePassword", "Change password") : t("account.setPassword", "Set password")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-16">
        {email && <input type="email" name="username" autoComplete="username" value={email} readOnly hidden />}
        {hasPassword && (
          <SensitiveInput
            kind="password"
            label={t("account.currentPassword", "Current password")}
            autoComplete="current-password"
            value={current}
            onChange={(e) => setCurrent(e.target.value)}
            autoFocus
            error={fieldError(save.error, "current_password")}
          />
        )}
        <SensitiveInput
          kind="password"
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

interface TwoFactorState {
  open: boolean;
  codes: string[] | null;
}

const TWO_FACTOR_CLOSED: TwoFactorState = { open: false, codes: null };

function TwoFactorSection({ enabled, codesLeft }: { enabled: boolean; codesLeft: number }) {
  const queryClient = useQueryClient();
  const [twoFactor, setTwoFactor] = useState<TwoFactorState>(TWO_FACTOR_CLOSED);
  const closeTwoFactor = useCallback(() => setTwoFactor(TWO_FACTOR_CLOSED), []);
  const [turningOff, setTurningOff] = useState(false);
  const [regenerating, setRegenerating] = useState(false);
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
      description={t("account.twoFactorHint", "A code at each password or link sign-in; SSO leaves it to your provider.")}
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
            description={t("account.authenticatorOff", "Off; signing in needs only the password or a sign-in link.")}
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
          onClose={closeTwoFactor}
          onEnabled={() => {
            void queryClient.invalidateQueries({ queryKey: ["me"] });
            void queryClient.invalidateQueries({ queryKey: ["session"] });
          }}
        />
      </DialogScope>
      <DialogScope open={turningOff}>
        <TurnOffDialog open={turningOff} onOpenChange={setTurningOff} />
      </DialogScope>
      <AlertDialog
        open={regenerating}
        onOpenChange={setRegenerating}
        tone="warning"
        title={t("account.regenerateConfirm", "Regenerate recovery codes?")}
        description={t("account.regenerateConfirmHint", "Your current recovery codes stop working; you get ten new ones to store.")}
        confirmLabel={t("account.regenerate", "Regenerate")}
        loading={regenerate.isPending}
        onConfirm={() => regenerate.mutateAsync()}
      />
    </SettingsSection>
  );
}

function TurnOffDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const [factor, setFactor] = useState<SecondFactorValue>(NO_SECOND_FACTOR);
  const off = useApiMutation({
    mutationFn: async () => {
      await api.DELETE("/platform/v1/me/totp", { body: secondFactorBody(factor) });
    },
    invalidates: [["me"], ["session"]],
    onSuccess: () => {
      onOpenChange(false);
      toast(t("account.twoFactorTurnedOff", "Two-factor authentication turned off"));
    },
    onError: () => setFactor((f) => ({ ...f, code: "" })),
  });
  const turnOff = () => secondFactorReady(factor) && !off.isPending && off.mutate();
  return (
    <Dialog
      busy={off.isPending}
      open={open}
      onOpenChange={onOpenChange}
      size="sm"
      title={t("account.turnOffConfirm", "Turn off two-factor authentication?")}
      description={t("account.turnOffConfirmHint", "Signing in needs only the password or a sign-in link; organizations that require it close to you.")}
      onSubmit={turnOff}
      footer={
        <>
          <Dialog.Close>
            <Button>{t("common.cancel", "Cancel")}</Button>
          </Dialog.Close>
          <Button variant="danger" disabled={!secondFactorReady(factor)} loading={off.isPending} onClick={turnOff}>
            {t("account.turnOff", "Turn off")}
          </Button>
        </>
      }
    >
      <SecondFactorField value={factor} onChange={setFactor} error={fieldError(off.error, "code")} />
    </Dialog>
  );
}

async function beginTotp() {
  try {
    return data(await api.POST("/platform/v1/me/totp"));
  } catch (error) {
    if (isApiError(error) && error.type === "recent_sign_in_required" && (await requestRecentSignIn())) {
      return data(await api.POST("/platform/v1/me/totp"));
    }
    throw error;
  }
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
    queryFn: beginTotp,
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
  const turnOn = (value: string) => {
    if (!confirm.isPending) confirm.mutate(value);
  };
  const [stored, setStored] = useState(false);
  const failed = start.isError;
  const closedOnFailure = useRef(false);
  useEffect(() => {
    if (!failed || closedOnFailure.current) return;
    closedOnFailure.current = true;
    onClose();
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
      busy={confirm.isPending}
      open={open}
      onOpenChange={(o) => !o && onClose()}
      size="md"
      title={t("account.setupTitle", "Set up two-factor authentication")}
      description={t("account.setupHint", "Add Kaeso to an authenticator app, then enter the 6-digit code it shows.")}
      onSubmit={() => code.length === 6 && turnOn(code)}
      footer={
        <>
          <Dialog.Close>
            <Button>{t("common.cancel", "Cancel")}</Button>
          </Dialog.Close>
          <Button variant="primary" disabled={code.length < 6} loading={confirm.isPending} onClick={() => turnOn(code)}>
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
                  <Code>{groupsOfFour(s.secret)}</Code>
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
          <OTPInput aria-label={t("account.appCode", "Code from the app")} value={code} onValueChange={setCode} onComplete={turnOn} />
        </Field>
      </div>
    </Dialog>
  );
}

const groupsOfFour = (secret: string) => secret.replace(/(.{4})(?=.)/g, "$1 ");

const LINKED_PAGE = "/settings/account/security";

function SingleSignOnSection() {
  const links = useQuery(ssoLinksQuery);
  const [unlinking, setUnlinking] = useState(false);
  const [target, setTarget] = useState({ org_id: "", org_name: "" });
  const link = useApiMutation({
    mutationFn: async (org: string) => data(await api.POST("/platform/v1/me/sso-links", { body: { org, next: LINKED_PAGE } })),
    onSuccess: (r) => window.location.assign(r.authorization_url),
  });
  const unlink = useApiMutation({
    mutationFn: async (org: string) => {
      await api.DELETE("/platform/v1/me/sso-links/{org}", { params: { path: { org } } });
    },
    invalidates: [ssoLinksQuery.queryKey],
    onSuccess: () => {
      setUnlinking(false);
      toast(t("account.ssoUnlinked", "Unlinked from single sign-on"));
    },
    onError: () => setUnlinking(false),
  });
  const rows = links.data?.data ?? [];
  if (rows.length === 0) return null;
  return (
    <SettingsSection
      title={t("account.sso", "Single sign-on")}
      description={t("account.ssoHint", "Only a linked account signs in through an organization's identity provider.")}
    >
      <SettingsGroup>
        {rows.map((r) => (
          <SettingsRow
            key={r.org_id}
            title={r.org_name}
            description={
              r.linked
                ? t("account.ssoLinked", "Linked; you can sign in through {provider}.", { provider: r.provider_name ?? t("account.ssoProvider", "its SSO") })
                : t("account.ssoNotLinked", "Not linked; link it to sign in through {provider}.", {
                    provider: r.provider_name ?? t("account.ssoProvider", "its SSO"),
                  })
            }
            control={
              r.linked ? (
                <Button
                  size="sm"
                  onClick={() => {
                    setTarget({ org_id: r.org_id, org_name: r.org_name });
                    setUnlinking(true);
                  }}
                >
                  {t("account.ssoUnlink", "Unlink")}
                </Button>
              ) : (
                <Button size="sm" loading={link.isPending && link.variables === r.org_id} onClick={() => link.mutate(r.org_id)}>
                  {t("account.ssoLink", "Link")}
                </Button>
              )
            }
          />
        ))}
      </SettingsGroup>
      <AlertDialog
        open={unlinking}
        onOpenChange={setUnlinking}
        tone="warning"
        title={t("account.ssoUnlinkConfirm", "Unlink from {org}'s single sign-on?", { org: target.org_name })}
        description={t("account.ssoUnlinkConfirmHint", "Its identity provider stops signing you in until you link the account again.")}
        confirmLabel={t("account.ssoUnlink", "Unlink")}
        loading={unlink.isPending}
        onConfirm={() => unlink.mutateAsync(target.org_id)}
      />
    </SettingsSection>
  );
}
