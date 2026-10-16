import { Suspense, lazy, useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Button, Input } from "@logicplanes/kaeso-ui";
import { PublicPage } from "~/shell/public-page";
import { toast } from "~/shell/toast";
import { isApiError } from "~/api/errors";
import { sendMagicLink, signInMethodsFor, signInWithPassword, signInWithTotp, startSso, type SignInMethods } from "~/auth/sign-in-api";
import { usePageTitle } from "~/shell/title";
import { safeNext } from "~/auth/next";
import { t } from "~/i18n";
import type { Busy } from "./sign-in-steps";

const loadSteps = () => import("./sign-in-steps");
const MethodsStep = lazy(() => loadSteps().then((m) => ({ default: m.MethodsStep })));
const TotpStep = lazy(() => loadSteps().then((m) => ({ default: m.TotpStep })));

type SignUpLine = typeof import("./sign-up-line").SignUpLine;
let signUpLine: SignUpLine | undefined;
const loadSignUpLine = () => import("./sign-up-line").then((m) => (signUpLine = m.SignUpLine));

type Step =
  | { kind: "email" }
  | { kind: "methods"; methods: SignInMethods }
  | { kind: "totp"; challenge: string; expiresAt: string; recovery: boolean }
  | { kind: "link-sent"; message: string };

export function SignInPage({ next, ssoError, notice }: { next?: string | undefined; ssoError?: string | null | undefined; notice?: string | undefined }) {
  usePageTitle(t("signIn.title", "Sign in"));
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [SignUp, setSignUp] = useState(() => signUpLine);
  useEffect(() => {
    if (!SignUp)
      loadSignUpLine().then(
        (line) => setSignUp(() => line),
        () => undefined,
      );
  }, [SignUp]);
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [code, setCode] = useState("");
  const [step, setStep] = useState<Step>({ kind: "email" });
  const [busy, setBusy] = useState<Busy | null>(null);
  const passwordFailed = useRef(false);
  useEffect(() => {
    if (busy || !passwordFailed.current) return;
    passwordFailed.current = false;
    const field = document.querySelector<HTMLInputElement>('input[autocomplete="current-password"]');
    field?.focus();
    field?.select();
  }, [busy]);
  useEffect(() => {
    if (ssoError) toast.error(ssoError, { id: "sso-error" });
  }, [ssoError]);
  useEffect(() => {
    if (!notice) return;
    toast.success(notice, { id: "sign-in-notice" });
    void navigate({ to: "/sign-in", search: next ? { next } : {}, replace: true });
  }, [notice, next, navigate]);

  const done = async () => {
    await queryClient.resetQueries();
    await navigate({ to: safeNext(next) });
  };
  const run = async (what: Busy, fn: () => Promise<void>) => {
    setBusy(what);
    try {
      await fn();
    } catch (e) {
      if (what === "code") setCode("");
      if (what === "password") passwordFailed.current = true;
      toast.error(isApiError(e) ? e.message : t("common.unreachable", "Can't reach the gateway. Try again."));
    } finally {
      setBusy(null);
    }
  };
  const address = email.trim();
  const withEmail = () =>
    run("continue", async () => {
      const [methods] = await Promise.all([signInMethodsFor(address), loadSteps()]);
      setStep({ kind: "methods", methods });
    });
  const withPassword = () =>
    run("password", async () => {
      const result = await signInWithPassword(address, password);
      if (result.kind === "totp") setStep({ kind: "totp", challenge: result.challenge, expiresAt: result.expiresAt, recovery: false });
      else await done();
    });
  const withCode = (totp: Extract<Step, { kind: "totp" }>, value = code) =>
    run("code", async () => {
      await signInWithTotp(totp.challenge, totp.recovery ? { recoveryCode: value.trim() } : { code: value });
      await done();
    });
  const withLink = () =>
    run("magic_link", async () => {
      await sendMagicLink(address);
      setStep({ kind: "link-sent", message: t("signIn.linkSent", "If an account exists for {email}, a sign-in link is on its way.", { email: address }) });
    });
  const withSignUp = () =>
    run("sign_up", async () => {
      const { requestSignUp } = await loadSteps();
      setStep({ kind: "link-sent", message: await requestSignUp(address) });
    });
  const withSso = () =>
    run("sso", async () => {
      window.location.assign(await startSso({ email: address }));
    });
  const otherEmail = () => {
    setEmail(address);
    setPassword("");
    setStep({ kind: "email" });
  };

  if (step.kind === "link-sent") {
    return (
      <PublicPage title={t("signIn.checkEmail", "Check your email")} description={step.message}>
        <Button size="xl" fullWidth onClick={otherEmail}>
          {t("signIn.back", "Back to sign in")}
        </Button>
      </PublicPage>
    );
  }

  if (step.kind === "totp") {
    return (
      <Suspense fallback={null}>
        <TotpStep
          recovery={step.recovery}
          expiresAt={step.expiresAt}
          code={code}
          onCodeChange={setCode}
          verifying={busy === "code"}
          onVerify={(value) => void withCode(step, value)}
          onToggleRecovery={() => (setCode(""), setStep({ ...step, recovery: !step.recovery }))}
        />
      </Suspense>
    );
  }

  if (step.kind === "methods") {
    return (
      <Suspense fallback={null}>
        <MethodsStep
          address={address}
          methods={step.methods}
          password={password}
          onPasswordChange={setPassword}
          busy={busy}
          onPassword={() => void withPassword()}
          onSso={() => void withSso()}
          onLink={() => void withLink()}
          onSignUp={() => void withSignUp()}
          onOtherEmail={otherEmail}
        />
      </Suspense>
    );
  }

  return (
    <PublicPage title={t("signIn.title", "Sign in")}>
      <form className="flex flex-col gap-16" onSubmit={(e) => (e.preventDefault(), void withEmail())}>
        <Input
          aria-label={t("signIn.email", "Email")}
          placeholder={t("signIn.emailAddress", "Email address")}
          size="xl"
          type="email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          autoComplete="username"
          autoFocus
        />
        <Button type="submit" variant="primary" size="xl" fullWidth loading={busy === "continue"} disabled={!address}>
          {t("common.continue", "Continue")}
        </Button>
      </form>
      {SignUp && <SignUp />}
    </PublicPage>
  );
}
