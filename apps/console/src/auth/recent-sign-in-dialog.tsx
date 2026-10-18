import { useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Button, Dialog, OTPInput, SensitiveInput, Text } from "@logicplanes/kaeso-ui";
import { toastApiError } from "~/api/toast-error";
import { meQuery } from "~/api/queries/me";
import { t } from "~/i18n";
import { settleRecentSignIn } from "./recent-sign-in-store";
import { sessionQuery, useSession } from "./session";
import { here, sendMagicLink, signInMethodsFor, signInWithPassword, signInWithTotp, startSso } from "./sign-in-api";

type Step = { kind: "password" } | { kind: "totp"; challenge: string; recovery: boolean } | { kind: "link-sent"; message: string };
type Way = "password" | "sso" | "link" | "none";

export function waysToSignIn(options: {
  email: string;
  hasPassword: boolean;
  ssoSession: boolean;
  methods: { password: boolean; magic_link: boolean; sso: boolean; sso_required: boolean } | undefined;
}): { primary: Way; sso: boolean; link: boolean } {
  const { email, hasPassword, ssoSession, methods } = options;
  const sso = ssoSession || !email || !!methods?.sso;
  if (methods?.sso_required) return { primary: "sso", sso: false, link: false };
  const password = hasPassword && !!email && (methods?.password ?? true);
  const link = !!email && (methods?.magic_link ?? true);
  if (password) return { primary: "password", sso, link };
  if (sso) return { primary: "sso", sso: false, link };
  if (link) return { primary: "link", sso: false, link: false };
  return { primary: "none", sso: false, link: false };
}

export default function RecentSignInDialog() {
  const session = useSession();
  const me = useQuery(meQuery);
  const queryClient = useQueryClient();
  const [step, setStep] = useState<Step>({ kind: "password" });
  const [password, setPassword] = useState("");
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const email = session.user.email ?? "";
  const methods = useQuery({ queryKey: ["auth", "methods", "for", email], queryFn: () => signInMethodsFor(email), enabled: !!email, retry: false });
  const loading = me.isPending || (!!email && methods.isPending);
  const ways = waysToSignIn({
    email,
    hasPassword: me.data?.has_password ?? false,
    ssoSession: session.session.method === "oidc",
    methods: methods.data,
  });
  const hasPassword = ways.primary === "password";

  const finish = async () => {
    await queryClient.invalidateQueries({ queryKey: sessionQuery.queryKey });
    settleRecentSignIn(true);
  };
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
      window.location.assign(await startSso({ email, next: here() }));
    });
  const emailLink = () =>
    attempt(async () => {
      setStep({ kind: "link-sent", message: await sendMagicLink(email) });
    });

  const cancel = () => settleRecentSignIn(false);
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
        {step.kind === "password" && (loading || ways.primary === "password") && (
          <Button variant="primary" loading={busy || loading} disabled={!password} onClick={submitPassword}>
            {t("auth.recent.signIn", "Sign in")}
          </Button>
        )}
        {step.kind === "password" && !loading && (ways.primary === "sso" || ways.primary === "link") && (
          <Button variant="primary" loading={busy} onClick={ways.primary === "sso" ? continueWithSso : emailLink}>
            {ways.primary === "sso" ? t("auth.continueWithSso", "Continue with SSO") : t("auth.recent.emailLink", "Email me a sign-in link")}
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
      title={t("auth.recent.title", "Sign in again to continue")}
      description={t("auth.recent.description", "This action needs a sign-in from the last 15 minutes.")}
      onSubmit={step.kind === "password" && !loading && hasPassword ? submitPassword : step.kind === "totp" ? () => submitCode(step) : undefined}
      footer={footer}
    >
      <div className="flex flex-col gap-16">
        {step.kind === "password" && loading && <Text tone="secondary">{t("auth.recent.checking", "Checking how you can sign in…")}</Text>}
        {step.kind === "password" && !loading && hasPassword && (
          <>
            <SensitiveInput
              label={t("auth.recent.password", "Password")}
              kind="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete="current-password"
              autoFocus
            />
            {ways.sso && (
              <Button variant="link" className="self-start" disabled={busy} onClick={() => void continueWithSso()}>
                {t("auth.recent.ssoInstead", "Continue with SSO instead")}
              </Button>
            )}
            {ways.link && (
              <Button variant="link" className="self-start" disabled={busy} onClick={() => void emailLink()}>
                {t("auth.recent.linkInstead", "Email me a sign-in link instead")}
              </Button>
            )}
          </>
        )}
        {step.kind === "password" && !loading && ways.primary === "sso" && (
          <Text tone="secondary">
            {email
              ? t("auth.recent.ssoHint", "Sign in again at your identity provider as {email}. You come back to this page.", { email })
              : t("auth.recent.ssoHintNoEmail", "Sign in again at your identity provider. You come back to this page.")}
          </Text>
        )}
        {step.kind === "password" && !loading && ways.primary === "link" && (
          <Text tone="secondary">{t("auth.recent.linkHint", "We send a sign-in link to {email}. Open it, then repeat the action.", { email })}</Text>
        )}
        {step.kind === "password" && !loading && ways.primary === "none" && (
          <Text tone="secondary">{t("auth.recent.noWay", "No sign-in method this install offers works for your account; ask an instance admin.")}</Text>
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
