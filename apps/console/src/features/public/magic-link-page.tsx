import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Button, Spinner, Text } from "@logicplanes/kaeso-ui";
import { PublicPage } from "~/shell/public-page";
import { api, data } from "~/api/client";
import { toastApiError } from "~/api/toast-error";
import { usePageTitle } from "~/shell/title";
import { safeNext } from "~/auth/next";
import { forgetFragment, takeFragment } from "~/auth/fragment";
import { sessionQuery } from "~/auth/session-queries";
import { codeRefusal, signInWithTotp, totpChallenge, type PasswordResult, type TotpChallenge } from "~/auth/sign-in-api";
import { t } from "~/i18n";

type Steps = typeof import("~/features/sign-in/sign-in-steps");

async function signInWithMagicLink(token: string): Promise<PasswordResult> {
  const { data, response } = await api.POST("/platform/v1/auth/magic-link/verify", { body: { token } });
  if (response.status === 202 && data && "challenge" in data) return { kind: "totp", ...totpChallenge(data) };
  return { kind: "signed_in" };
}
type Totp = TotpChallenge & { recovery: boolean; failures: number };

function Waiting() {
  return (
    <PublicPage title={t("public.signingYouInNow", "Signing you in…")}>
      <div className="flex items-center justify-center gap-8 show-after-delay">
        <Spinner label={t("public.signingYouIn", "Signing you in")} />
        <Text tone="secondary">{t("public.oneMoment", "One moment.")}</Text>
      </div>
    </PublicPage>
  );
}

export function MagicLinkPage({ next }: { next?: string | undefined }) {
  usePageTitle(t("public.signingIn", "Signing in"));
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [token] = useState(() => takeFragment("token") ?? "");
  const account = useQuery({
    queryKey: ["magic-link", token],
    queryFn: async () => {
      try {
        return data(await api.POST("/platform/v1/auth/magic-link/lookup", { body: { token } }));
      } catch (e) {
        toastApiError(e, { id: "token-link" });
        throw e;
      }
    },
    enabled: !!token,
    retry: false,
    staleTime: Infinity,
    meta: { quiet: true },
  });
  const current = useQuery({ ...sessionQuery, retry: false, meta: { quiet: true } }).data;
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);
  const [totp, setTotp] = useState<Totp | null>(null);
  const [steps, setSteps] = useState<Steps | null>(null);
  const [code, setCode] = useState("");
  const done = async () => {
    forgetFragment(token);
    await queryClient.resetQueries();
    await navigate({ to: safeNext(next), replace: true });
    queryClient.removeQueries({ queryKey: ["magic-link", token], exact: true });
  };
  const run = async (fn: () => Promise<void>, onError: (e: unknown) => void) => {
    setBusy(true);
    try {
      await fn();
    } catch (e) {
      toastApiError(e, { id: "token-link" });
      onError(e);
    } finally {
      setBusy(false);
    }
  };
  const signIn = () =>
    run(
      async () => {
        const result = await signInWithMagicLink(token);
        if (result.kind === "signed_in") return done();
        setSteps(await import("~/features/sign-in/sign-in-steps"));
        setTotp({ ...result, recovery: result.codesLocked, failures: 0 });
      },
      () => setFailed(true),
    );
  const verify = (step: Totp, value = code) =>
    run(
      async () => {
        await signInWithTotp(step.challenge, step.recovery ? { recoveryCode: value.trim() } : { code: value });
        await done();
      },
      (e) => {
        setCode("");
        const failures = step.failures + 1;
        const refusal = codeRefusal(e, step, failures);
        if (refusal === "ended") setFailed(true);
        else if (refusal === "locked") setTotp({ ...step, failures, recovery: true, codesLocked: true });
        else setTotp({ ...step, failures });
      },
    );

  if (!token) {
    return (
      <PublicPage
        title={t("public.needsSignInLink", "This page needs the sign-in link")}
        description={t("public.needsSignInLinkHint", "Open the link from the sign-in email; it ends with #token=…")}
      >
        <Button size="xl" fullWidth href="/sign-in">
          {t("public.backToSignIn", "Back to sign in")}
        </Button>
      </PublicPage>
    );
  }
  if (failed || account.isError) {
    return (
      <PublicPage
        title={t("public.linkUsed", "This link has expired or was already used.")}
        description={t("public.signInLinkOnce", "A sign-in link works once, for 15 minutes.")}
      >
        <Button variant="primary" size="xl" fullWidth href="/sign-in">
          {t("public.requestNewLink", "Request a new link")}
        </Button>
      </PublicPage>
    );
  }
  if (totp && steps) {
    const { TotpStep } = steps;
    return (
      <TotpStep
        recovery={totp.recovery}
        codesLocked={totp.codesLocked}
        expiresAt={totp.expiresAt}
        code={code}
        onCodeChange={setCode}
        verifying={busy}
        onVerify={(value) => void verify(totp, value)}
        onToggleRecovery={() => (setCode(""), setTotp({ ...totp, recovery: !totp.recovery }))}
      />
    );
  }
  if (!account.data) return <Waiting />;
  const email = account.data.email;
  const signedInAs = current?.user.email ?? null;
  const same = !!signedInAs && signedInAs.toLowerCase() === email.toLowerCase();
  return (
    <PublicPage
      title={t("public.signInAs", "Sign in as {email}", { email })}
      description={
        same
          ? t("public.alreadySignedInAs", "You're already signed in as {email}; the link signs you in again here.", { email })
          : signedInAs
            ? t("public.replacesSession", "You're signed in as {current} in this browser; signing in as {email} replaces that here.", {
                current: signedInAs,
                email,
              })
            : t("public.signInAsHint", "Continue only if you asked for this link yourself.")
      }
    >
      <div className="flex flex-col gap-12">
        <Button variant="primary" size="xl" fullWidth loading={busy} onClick={() => void signIn()}>
          {t("public.signIn", "Sign in")}
        </Button>
        {signedInAs && (
          <Button size="xl" fullWidth href="/">
            {same ? t("public.keepGoing", "Keep this session") : t("public.staySignedInAs", "Stay signed in as {email}", { email: signedInAs })}
          </Button>
        )}
      </div>
    </PublicPage>
  );
}
