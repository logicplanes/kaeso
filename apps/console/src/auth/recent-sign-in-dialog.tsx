import { useCallback, useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { Button, Dialog, OTPInput, SensitiveInput, Text } from "@logicplanes/kaeso-ui";
import { toastApiError } from "~/api/toast-error";
import { t } from "~/i18n";
import { CONFIRMED_PATH, onConfirmed } from "./confirm-channel";
import { settleRecentSignIn, signInAgainKind } from "./recent-sign-in-store";
import { sessionQuery, useSession } from "./session";
import {
  codeRefusal,
  confirmWaysQuery,
  confirmWithSso,
  emailPasswordLink,
  here,
  sendMagicLink,
  signInWithPassword,
  signInWithTotp,
  type ConfirmSso,
  type ConfirmWays,
  type TotpChallenge,
} from "./sign-in-api";
import { showSsoError } from "./sso-errors";

type Step =
  | { kind: "choose" }
  | ({ kind: "totp"; recovery: boolean; failures: number } & TotpChallenge)
  | { kind: "link-sent"; message: string }
  | { kind: "sso-waiting"; provider: ConfirmSso };
type Way = "password" | "sso" | "link" | "set-password" | "none";

export function confirmChoices(
  ways: ConfirmWays | undefined,
  options: { withoutSso: boolean; ssoOrg: string | null | undefined },
): { primary: Way; sso: ConfirmSso[]; link: boolean } {
  if (!ways) return { primary: "none", sso: [], link: false };
  const sso = options.withoutSso ? [] : [...ways.sso].sort((a, b) => Number(b.org_id === options.ssoOrg) - Number(a.org_id === options.ssoOrg));
  if (ways.password) return { primary: "password", sso, link: ways.magic_link };
  if (sso.length) return { primary: "sso", sso, link: ways.magic_link };
  if (ways.magic_link) return { primary: "link", sso: [], link: false };
  if (ways.set_password) return { primary: "set-password", sso: [], link: false };
  return { primary: "none", sso: [], link: false };
}

const providerName = (p: ConfirmSso) => p.provider_name ?? p.org_name;

function openPopup(): Window | null {
  const width = 520;
  const height = 720;
  const left = Math.max(0, window.screenX + (window.outerWidth - width) / 2);
  const top = Math.max(0, window.screenY + (window.outerHeight - height) / 2);
  return window.open("", "kaeso-sign-in-again", `popup,width=${width},height=${height},left=${left},top=${top}`);
}

export default function RecentSignInDialog() {
  const [withoutSso] = useState(() => signInAgainKind() === "without-sso");
  const session = useSession();
  const { org } = useParams({ strict: false });
  const queryClient = useQueryClient();
  const confirm = useQuery(confirmWaysQuery(org));
  const [step, setStep] = useState<Step>({ kind: "choose" });
  const [password, setPassword] = useState("");
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const email = session.user.email ?? "";
  const loading = confirm.isPending;
  const ways = confirmChoices(confirm.data, { withoutSso, ssoOrg: session.session.sso_org });

  const finish = useCallback(async () => {
    await queryClient.invalidateQueries({ queryKey: sessionQuery.queryKey });
    settleRecentSignIn(true);
  }, [queryClient]);
  const waiting = step.kind === "sso-waiting";
  useEffect(() => {
    if (!waiting) return;
    return onConfirmed(({ error }) => {
      if (!error) return void finish();
      showSsoError(error, "sign-in-again");
      setStep({ kind: "choose" });
    });
  }, [waiting, finish]);
  const running = useRef(false);
  const attempt = async (fn: () => Promise<void>) => {
    if (running.current) return;
    running.current = true;
    setBusy(true);
    try {
      await fn();
    } catch (e) {
      toastApiError(e);
    } finally {
      running.current = false;
      setBusy(false);
    }
  };
  const submitPassword = () =>
    attempt(async () => {
      const result = await signInWithPassword(email, password);
      if (result.kind === "totp") setStep({ ...result, recovery: result.codesLocked, failures: 0 });
      else await finish();
    });
  const submitCode = (totp: Extract<Step, { kind: "totp" }>, value = code) =>
    attempt(async () => {
      try {
        await signInWithTotp(totp.challenge, totp.recovery ? { recoveryCode: value.trim() } : { code: value });
      } catch (e) {
        const failures = totp.failures + 1;
        const refusal = codeRefusal(e, totp, failures);
        if (refusal !== "again") setCode("");
        if (refusal === "ended") {
          setPassword("");
          setStep({ kind: "choose" });
        } else if (refusal === "locked") setStep({ ...totp, failures, recovery: true, codesLocked: true });
        else setStep({ ...totp, failures });
        throw e;
      }
      await finish();
    });
  const continueWithSso = (provider: ConfirmSso) => {
    const popup = openPopup();
    void attempt(async () => {
      try {
        const url = await confirmWithSso(provider.org_id, popup ? CONFIRMED_PATH : here());
        if (!popup) return window.location.assign(url);
        popup.location.href = url;
        popup.focus();
        setStep({ kind: "sso-waiting", provider });
      } catch (e) {
        popup?.close();
        throw e;
      }
    });
  };
  const emailLink = () =>
    attempt(async () => {
      setStep({ kind: "link-sent", message: await sendMagicLink(email) });
    });
  const emailPassword = () =>
    attempt(async () => {
      await emailPasswordLink(email);
      setStep({
        kind: "link-sent",
        message: t("auth.recent.setPasswordSent", "We sent a link to {email}. Set a password there, sign in with it, then repeat the action.", { email }),
      });
    });

  const cancel = () => settleRecentSignIn(false);
  const first = ways.sso[0];
  const others = ways.primary === "sso" ? ways.sso.slice(1) : ways.sso;
  const footer =
    step.kind === "link-sent" ? (
      <Dialog.Close>
        <Button>{t("common.close", "Close")}</Button>
      </Dialog.Close>
    ) : (
      <>
        <Dialog.Close>
          <Button>{t("common.cancel", "Cancel")}</Button>
        </Dialog.Close>
        {step.kind === "choose" && (loading || ways.primary === "password") && (
          <Button variant="primary" loading={busy || loading} disabled={!password} onClick={submitPassword}>
            {t("auth.recent.signIn", "Sign in")}
          </Button>
        )}
        {step.kind === "choose" && !loading && ways.primary === "sso" && first && (
          <Button variant="primary" loading={busy} onClick={() => continueWithSso(first)}>
            {t("auth.recent.continueWith", "Continue with {provider}", { provider: providerName(first) })}
          </Button>
        )}
        {step.kind === "choose" && !loading && ways.primary === "link" && (
          <Button variant="primary" loading={busy} onClick={emailLink}>
            {t("auth.recent.emailLink", "Email me a sign-in link")}
          </Button>
        )}
        {step.kind === "choose" && !loading && ways.primary === "set-password" && (
          <Button variant="primary" loading={busy} onClick={emailPassword}>
            {t("auth.recent.emailPassword", "Email me a link to set a password")}
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
      busy={busy}
      open
      onOpenChange={(o) => !o && cancel()}
      size="sm"
      title={withoutSso ? t("auth.other.title", "Sign in another way") : t("auth.recent.title", "Sign in again to continue")}
      description={
        withoutSso
          ? t("auth.other.description", "You signed in with SSO; this needs your password or an email link.")
          : t("auth.recent.description", "This action needs a sign-in from the last 15 minutes.")
      }
      onSubmit={
        step.kind === "choose" && !loading && ways.primary === "password" && password
          ? submitPassword
          : step.kind === "totp" && (step.recovery ? code.trim() : code.length === 6)
            ? () => submitCode(step)
            : undefined
      }
      footer={footer}
    >
      <div className="flex flex-col gap-16">
        {step.kind === "choose" && loading && <Text tone="secondary">{t("auth.recent.checking", "Checking how you can sign in…")}</Text>}
        {step.kind === "choose" && confirm.isError && (
          <>
            <Text tone="secondary">{t("auth.recent.checkFailed", "Kaeso couldn't check how you can sign in again.")}</Text>
            <Button variant="link" className="self-start" onClick={() => void confirm.refetch()}>
              {t("common.tryAgain", "Try again")}
            </Button>
          </>
        )}
        {step.kind === "choose" && !loading && ways.primary === "password" && (
          <SensitiveInput
            label={t("auth.recent.password", "Password")}
            kind="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            autoComplete="current-password"
            autoFocus
          />
        )}
        {step.kind === "choose" && !loading && ways.primary === "sso" && first && (
          <Text tone="secondary">
            {t("auth.recent.ssoHint", "Sign in again at {provider} as {email}, in a window that opens; this page stays as it is.", {
              provider: providerName(first),
              email: email || t("auth.recent.yourself", "yourself"),
            })}
          </Text>
        )}
        {step.kind === "choose" && !loading && ways.primary === "link" && (
          <Text tone="secondary">{t("auth.recent.linkHint", "We send a sign-in link to {email}. Open it, then repeat the action.", { email })}</Text>
        )}
        {step.kind === "choose" && !loading && ways.primary === "set-password" && (
          <Text tone="secondary">{t("auth.recent.setPasswordHint", "Your account has no password yet. We send a link to {email} to set one.", { email })}</Text>
        )}
        {step.kind === "choose" && !loading && !confirm.isError && ways.primary === "none" && (
          <Text tone="secondary">{t("auth.recent.noWay", "No sign-in method this install offers works for your account; ask an instance admin.")}</Text>
        )}
        {step.kind === "choose" && !loading && (others.length > 0 || (ways.primary !== "link" && ways.link)) && (
          <div className="flex flex-col items-start gap-4">
            {others.map((p) => (
              <Button key={p.org_id} variant="link" disabled={busy} onClick={() => continueWithSso(p)}>
                {t("auth.recent.continueWithInstead", "Continue with {provider} instead", { provider: providerName(p) })}
              </Button>
            ))}
            {ways.primary !== "link" && ways.link && (
              <Button variant="link" disabled={busy} onClick={() => void emailLink()}>
                {t("auth.recent.linkInstead", "Email me a sign-in link instead")}
              </Button>
            )}
          </div>
        )}
        {step.kind === "sso-waiting" && (
          <>
            <Text tone="secondary">
              {t("auth.recent.waiting", "Finish signing in at {provider} in the window that opened; the action continues by itself.", {
                provider: providerName(step.provider),
              })}
            </Text>
            <Button variant="link" className="self-start" disabled={busy} onClick={() => continueWithSso(step.provider)}>
              {t("auth.recent.openAgain", "Open the window again")}
            </Button>
          </>
        )}
        {step.kind === "totp" && !step.recovery && (
          <>
            <Text tone="secondary">{t("auth.recent.codeHint", "Enter the 6-digit code from your authenticator app.")}</Text>
            <OTPInput value={code} onValueChange={setCode} onComplete={(v) => submitCode(step, v)} autoFocus />
            <Button
              variant="link"
              className="self-start"
              disabled={busy}
              onClick={() => {
                setCode("");
                setStep({ ...step, recovery: true });
              }}
            >
              {t("auth.recent.useRecovery", "Use a recovery code")}
            </Button>
          </>
        )}
        {step.kind === "totp" && step.codesLocked && (
          <Text tone="secondary">{t("auth.recent.codesLocked", "Too many wrong codes locked your authenticator app; enter a recovery code.")}</Text>
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
