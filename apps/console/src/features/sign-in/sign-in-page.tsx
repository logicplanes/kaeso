import { useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Button, Input } from "@logicplanes/kaeso-ui";
import { PublicPage } from "~/shell/public-page";
import { toast } from "~/shell/toast";
import { toastApiError } from "~/api/toast-error";
import { isApiError, type ApiError } from "~/api/errors";
import {
  codeRefusal,
  sendMagicLink,
  signInMethodsFor,
  signInWithPassword,
  signInWithTotp,
  startSso,
  type SignInMethods,
  type TotpChallenge,
} from "~/auth/sign-in-api";
import { usePageTitle } from "~/shell/title";
import { handResetAddress } from "~/features/public/reset-address";
import { safeNext } from "~/auth/next";
import { t } from "~/i18n";
import type { Busy } from "./sign-in-steps";

type Steps = typeof import("./sign-in-steps");
let steps: Steps | undefined;
const loadSteps = () => import("./sign-in-steps").then((m) => (steps = m));

type SignUpLine = typeof import("./sign-up-line").SignUpLine;
let signUpLine: SignUpLine | undefined;
const loadSignUpLine = () => import("./sign-up-line").then((m) => (signUpLine = m.SignUpLine));

type Step =
  | { kind: "email" }
  | { kind: "methods"; methods: SignInMethods }
  | ({ kind: "totp"; recovery: boolean; failures: number } & TotpChallenge)
  | { kind: "link-sent"; message: string };

const refusedPassword = (e: unknown): e is ApiError => isApiError(e) && !e.unreachable && e.type !== "internal";

const onlySso = (m: SignInMethods) => m.sso && !m.password && !m.magic_link && !m.sign_up;

export function SignInPage({
  next,
  org,
  page,
  ssoError,
  notice,
  quiet = false,
}: {
  next?: string | undefined;
  org?: string | undefined;
  page?: SignInMethods | null | undefined;
  ssoError?: string | null | undefined;
  notice?: string | undefined;
  quiet?: boolean;
}) {
  usePageTitle(t("signIn.title", "Sign in"));
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const provider = page?.provider ?? null;
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
  const [passwordError, setPasswordError] = useState<string | null>(null);
  const [code, setCode] = useState("");
  const [step, setStep] = useState<Step>({ kind: "email" });
  const [withEmailInstead, setWithEmailInstead] = useState(false);
  const [loaded, setLoaded] = useState(() => steps);
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
    if (ssoError) void loadSteps().then((m) => m.showSsoError(ssoError));
  }, [ssoError]);
  useEffect(() => {
    if (!notice) return;
    toast.success(notice, { id: "sign-in-notice" });
    void navigate({ to: "/sign-in", search: { ...(next ? { next } : {}), ...(org ? { org } : {}) }, replace: true });
  }, [notice, next, org, navigate]);

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
      if (what === "password" && refusedPassword(e)) setPasswordError(e.message);
      else toastApiError(e);
    } finally {
      setBusy(null);
    }
  };
  const toProvider = async (body: { email: string } | { org: string }) => {
    window.location.assign(await startSso({ ...body, ...(next ? { next } : {}) }));
  };
  const address = email.trim();
  const withEmail = () =>
    run("continue", async () => {
      const [methods] = await Promise.all([signInMethodsFor(address), loadSteps()]);
      if (onlySso(methods) && methods.provider?.auto_redirect) await toProvider({ email: address });
      else setStep({ kind: "methods", methods });
    });
  const withPassword = () =>
    run("password", async () => {
      const result = await signInWithPassword(address, password);
      if (result.kind === "totp") setStep({ ...result, recovery: result.codesLocked, failures: 0 });
      else await done();
    });
  const withCode = (totp: Extract<Step, { kind: "totp" }>, value = code) =>
    run("code", async () => {
      try {
        await signInWithTotp(totp.challenge, totp.recovery ? { recoveryCode: value.trim() } : { code: value });
      } catch (e) {
        const failures = totp.failures + 1;
        const refusal = codeRefusal(e, totp, failures);
        if (refusal === "ended") otherEmail();
        else if (refusal === "locked") setStep({ ...totp, failures, recovery: true, codesLocked: true });
        else setStep({ ...totp, failures });
        throw e;
      }
      await done();
    });
  const withLink = () =>
    run("magic_link", async () => {
      await sendMagicLink(address);
      setStep({ kind: "link-sent", message: (await loadSteps()).linkSent(address) });
    });
  const withSignUp = () =>
    run("sign_up", async () => {
      const { requestSignUp } = await loadSteps();
      setStep({ kind: "link-sent", message: await requestSignUp(address) });
    });
  const withSso = () => run("sso", () => toProvider({ email: address }));
  const otherEmail = () => {
    setEmail(address);
    setPassword("");
    setPasswordError(null);
    setStep({ kind: "email" });
  };
  const alone = provider && !withEmailInstead && step.kind === "email" ? provider : null;
  useEffect(() => {
    if (provider && !loaded)
      loadSteps().then(
        (m) => setLoaded(m),
        () => setWithEmailInstead(true),
      );
  }, [provider, loaded]);

  if (step.kind === "link-sent") {
    return (
      <PublicPage title={t("signIn.checkEmail", "Check your email")} description={step.message}>
        <Button size="xl" fullWidth onClick={otherEmail} autoFocus>
          {t("signIn.back", "Back to sign in")}
        </Button>
      </PublicPage>
    );
  }

  if (step.kind === "totp" && steps) {
    const { TotpStep } = steps;
    return (
      <TotpStep
        recovery={step.recovery}
        codesLocked={step.codesLocked}
        expiresAt={step.expiresAt}
        code={code}
        onCodeChange={setCode}
        verifying={busy === "code"}
        onVerify={(value) => void withCode(step, value)}
        onToggleRecovery={() => (setCode(""), setStep({ ...step, recovery: !step.recovery }))}
      />
    );
  }

  if (step.kind === "methods" && steps) {
    const { MethodsStep } = steps;
    return (
      <MethodsStep
        address={address}
        methods={step.methods}
        password={password}
        passwordError={passwordError ?? undefined}
        onPasswordChange={(value) => (setPassword(value), setPasswordError(null))}
        busy={busy}
        onPassword={() => void withPassword()}
        onSso={() => void withSso()}
        onLink={() => void withLink()}
        onSignUp={() => void withSignUp()}
        onOtherEmail={otherEmail}
        onForgotPassword={() => (handResetAddress(address), void navigate({ to: "/auth/password-reset" }))}
      />
    );
  }

  if (alone && loaded) {
    const { ProviderStep } = loaded;
    return (
      <ProviderStep
        provider={alone}
        next={next}
        auto={!quiet}
        onEmail={page?.password || page?.magic_link ? () => setWithEmailInstead(true) : undefined}
        signUp={SignUp && <SignUp />}
      />
    );
  }

  if (page === undefined || alone) return <PublicPage title={t("signIn.title", "Sign in")}>{null}</PublicPage>;

  return (
    <PublicPage title={t("signIn.title", "Sign in")}>
      <form className="flex flex-col gap-16" onSubmit={(e) => (e.preventDefault(), void withEmail())}>
        <Input
          label={t("signIn.email", "Email")}
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
