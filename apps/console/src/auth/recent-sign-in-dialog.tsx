import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Button, Dialog, OTPInput, SensitiveInput, Text } from "@logicplanes/kaeso-ui";
import { toastApiError } from "~/api/toast-error";
import { meQuery } from "~/api/queries/me";
import { t } from "~/i18n";
import { RETURN_KEY, settleRecentSignIn } from "./recent-sign-in-store";
import { sessionQuery, useSession } from "./session";
import { sendMagicLink, signInWithPassword, signInWithTotp, startSso } from "./sign-in-api";
import { writeStored } from "~/storage";

type Step = { kind: "password" } | { kind: "totp"; challenge: string; recovery: boolean } | { kind: "link-sent"; message: string };

export default function RecentSignInDialog() {
  const session = useSession();
  const me = useQuery(meQuery);
  const queryClient = useQueryClient();
  const [step, setStep] = useState<Step>({ kind: "password" });
  const [password, setPassword] = useState("");
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const email = session.user.email ?? "";
  const sso = session.session.method === "oidc";
  const hasPassword = me.data?.has_password ?? true;

  const finish = async () => {
    await queryClient.invalidateQueries({ queryKey: sessionQuery.queryKey });
    settleRecentSignIn(true);
  };
  const attempt = async (fn: () => Promise<void>) => {
    setBusy(true);
    try {
      await fn();
    } catch (e) {
      toastApiError(e);
    } finally {
      setBusy(false);
    }
  };
  const submitPassword = () =>
    attempt(async () => {
      const result = await signInWithPassword(email, password);
      if (result.kind === "totp") setStep({ kind: "totp", challenge: result.challenge, recovery: false });
      else await finish();
    });
  const submitCode = (totp: Extract<Step, { kind: "totp" }>, value = code) =>
    attempt(async () => {
      await signInWithTotp(totp.challenge, totp.recovery ? { recoveryCode: value.trim() } : { code: value });
      await finish();
    });
  const continueWithSso = () =>
    attempt(async () => {
      writeStored(RETURN_KEY, window.location.pathname + window.location.search, "session");
      window.location.assign(await startSso({ email }));
    });
  const emailLink = () =>
    attempt(async () => {
      setStep({ kind: "link-sent", message: await sendMagicLink(email) });
    });

  const cancel = () => settleRecentSignIn(false);
  const footer =
    step.kind === "link-sent" ? (
      <Button onClick={cancel}>{t("common.close", "Close")}</Button>
    ) : (
      <>
        <Button onClick={cancel}>{t("common.cancel", "Cancel")}</Button>
        {step.kind === "password" && hasPassword && (
          <Button variant="primary" loading={busy} disabled={!password} onClick={submitPassword}>
            {t("auth.recent.signIn", "Sign in")}
          </Button>
        )}
        {step.kind === "password" && !hasPassword && (
          <Button variant="primary" loading={busy} onClick={sso ? continueWithSso : emailLink}>
            {sso ? t("auth.continueWithSso", "Continue with SSO") : t("auth.recent.emailLink", "Email me a sign-in link")}
          </Button>
        )}
        {step.kind === "totp" && (
          <Button variant="primary" loading={busy} disabled={step.recovery ? !code.trim() : code.length < 6} onClick={() => submitCode(step)}>
            {t("auth.recent.verify", "Verify")}
          </Button>
        )}
      </>
    );

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && cancel()}
      size="sm"
      title={t("auth.recent.title", "Sign in again to continue")}
      description={t("auth.recent.description", "This action needs a sign-in from the last 15 minutes.")}
      onSubmit={step.kind === "password" && hasPassword ? submitPassword : step.kind === "totp" ? () => submitCode(step) : undefined}
      footer={footer}
    >
      <div className="flex flex-col gap-16">
        {step.kind === "password" && hasPassword && (
          <>
            <SensitiveInput
              label={t("auth.recent.password", "Password")}
              kind="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete="current-password"
              autoFocus
            />
            {sso && (
              <Button variant="link" className="self-start" onClick={() => void continueWithSso()}>
                {t("auth.recent.ssoInstead", "Continue with SSO instead")}
              </Button>
            )}
          </>
        )}
        {step.kind === "password" && !hasPassword && (
          <Text tone="secondary">
            {sso
              ? t("auth.recent.ssoHint", "Sign in again at your identity provider as {email}. You come back to this page.", { email })
              : t("auth.recent.linkHint", "We send a sign-in link to {email}. Open it, then repeat the action.", { email })}
          </Text>
        )}
        {step.kind === "totp" && !step.recovery && (
          <>
            <Text tone="secondary">{t("auth.recent.codeHint", "Enter the 6-digit code from your authenticator app.")}</Text>
            <OTPInput value={code} onValueChange={setCode} onComplete={(v) => submitCode(step, v)} autoFocus />
            <Button
              variant="link"
              className="self-start"
              onClick={() => {
                setCode("");
                setStep({ ...step, recovery: true });
              }}
            >
              {t("auth.recent.useRecovery", "Use a recovery code")}
            </Button>
          </>
        )}
        {step.kind === "totp" && step.recovery && (
          <SensitiveInput
            label={t("auth.recent.recoveryCode", "Recovery code")}
            kind="secret"
            value={code}
            onChange={(e) => setCode(e.target.value)}
            autoComplete="one-time-code"
            autoFocus
          />
        )}
        {step.kind === "link-sent" && <Text tone="secondary">{step.message}</Text>}
      </div>
    </Dialog>
  );
}
