import { Button, Input, Link, OTPInput, SensitiveInput, Text } from "@logicplanes/kaeso-ui";
import { formatRelative } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { toastApiError } from "~/api/toast-error";
import { startSso, type SignInMethods, type SsoProvider } from "~/auth/sign-in-api";
import { PublicPage } from "~/shell/public-page";
import { t } from "~/i18n";
import { tRich } from "~/i18n/rich";
export { showSsoError } from "~/auth/sso-errors";

export const linkSent = (email: string): string => t("signIn.linkSent", "If an account exists for {email}, a sign-in link is on its way.", { email });

export type Busy = "continue" | "password" | "magic_link" | "sso" | "code" | "sign_up";

export async function requestSignUp(email: string): Promise<string> {
  return data(await api.POST("/platform/v1/auth/signup", { body: { email } })).message;
}

function ssoLabel(provider: SsoProvider | null | undefined): string {
  return provider?.name ? t("signIn.ssoWith", "Continue with {provider}", { provider: provider.name }) : t("signIn.sso", "Continue with SSO");
}

export function ProviderStep({
  provider,
  next,
  auto,
  onEmail,
  signUp,
}: {
  provider: SsoProvider;
  next: string | undefined;
  auto: boolean;
  onEmail: (() => void) | undefined;
  signUp: ReactNode;
}) {
  const [starting, setStarting] = useState(false);
  const started = useRef(false);
  const onStart = async () => {
    setStarting(true);
    try {
      window.location.assign(await startSso({ org: provider.org, ...(next ? { next } : {}) }));
    } catch (e) {
      toastApiError(e);
    } finally {
      setStarting(false);
    }
  };
  useEffect(() => {
    if (!auto || !provider.auto_redirect || started.current) return;
    started.current = true;
    void onStart();
  });
  return (
    <PublicPage title={t("signIn.title", "Sign in")}>
      <div className="flex flex-col gap-16">
        <Button variant="primary" size="xl" fullWidth loading={starting} onClick={() => void onStart()} autoFocus>
          {ssoLabel(provider)}
        </Button>
        {onEmail && (
          <Text variant="small" tone="secondary" className="text-center">
            <Link href="#" variant="quiet" onClick={(e) => (e.preventDefault(), onEmail())}>
              {t("signIn.useEmail", "Sign in with your email instead")}
            </Link>
          </Text>
        )}
      </div>
      {signUp}
    </PublicPage>
  );
}

export function MethodsStep({
  address,
  methods: m,
  password,
  onPasswordChange,
  busy,
  onPassword,
  onSso,
  onLink,
  onSignUp,
  onOtherEmail,
}: {
  address: string;
  methods: SignInMethods;
  password: string;
  onPasswordChange: (password: string) => void;
  busy: Busy | null;
  onPassword: () => void;
  onSso: () => void;
  onLink: () => void;
  onSignUp: () => void;
  onOtherEmail: () => void;
}) {
  const none = !m.password && !m.magic_link && !m.sso;
  return (
    <PublicPage
      title={t("signIn.title", "Sign in")}
      description={
        none && !m.sign_up ? t("signIn.noMethod", "There's no way to sign in with {email} on this install. Ask an administrator.", { email: address }) : address
      }
    >
      <form className="flex flex-col gap-16" onSubmit={(e) => (e.preventDefault(), m.password ? onPassword() : undefined)}>
        <input type="email" name="username" autoComplete="username" value={address} readOnly hidden />
        {m.sso && (
          <Button variant={m.password ? "secondary" : "primary"} size="xl" fullWidth loading={busy === "sso"} onClick={onSso} autoFocus={!m.password}>
            {ssoLabel(m.provider)}
          </Button>
        )}
        {m.password && (
          <>
            <SensitiveInput
              label={t("signIn.password", "Password")}
              kind="password"
              size="xl"
              value={password}
              onChange={(e) => onPasswordChange(e.target.value)}
              autoComplete="current-password"
              autoFocus
            />
            <Button type="submit" variant="primary" size="xl" fullWidth loading={busy === "password"} disabled={!password}>
              {t("signIn.title", "Sign in")}
            </Button>
          </>
        )}
        {m.magic_link && (
          <Button size="xl" fullWidth loading={busy === "magic_link"} onClick={onLink} autoFocus={!m.password && !m.sso}>
            {t("signIn.emailLink", "Email me a sign-in link")}
          </Button>
        )}
        <Text variant="small" tone="secondary" className="text-center">
          {m.password && (
            <>
              <Link href="/auth/password-reset" variant="quiet">
                {t("signIn.forgotPassword", "Forgot your password?")}
              </Link>
              {" · "}
            </>
          )}
          <Link href="#" variant="quiet" onClick={(e) => (e.preventDefault(), onOtherEmail())}>
            {t("signIn.otherEmail", "Use another email")}
          </Link>
        </Text>
        {m.sign_up && (
          <Text variant="small" tone="secondary" className="text-center">
            {tRich("signIn.newHere", "New here? {link}", {
              link: (
                <Link href="#" aria-disabled={busy === "sign_up"} onClick={(e) => (e.preventDefault(), busy ? undefined : onSignUp())}>
                  {t("signIn.createAccount", "Create an account")}
                </Link>
              ),
            })}
          </Text>
        )}
      </form>
    </PublicPage>
  );
}

export function TotpStep({
  recovery,
  expiresAt,
  code,
  onCodeChange,
  verifying,
  onVerify,
  onToggleRecovery,
}: {
  recovery: boolean;
  expiresAt: string;
  code: string;
  onCodeChange: (code: string) => void;
  verifying: boolean;
  onVerify: (value?: string) => void;
  onToggleRecovery: () => void;
}) {
  return (
    <PublicPage
      title={t("signIn.twoStep", "Two-step verification")}
      description={
        recovery
          ? t("signIn.recoveryHint", "Enter one of your recovery codes; each works once.")
          : t("signIn.codeHint", "Enter the 6-digit code from your authenticator app.")
      }
    >
      <form className="flex flex-col gap-16" onSubmit={(e) => (e.preventDefault(), onVerify())}>
        {recovery ? (
          <Input
            label={t("signIn.recoveryCode", "Recovery code")}
            size="xl"
            value={code}
            onChange={(e) => onCodeChange(e.target.value)}
            autoComplete="one-time-code"
            autoFocus
            spellCheck={false}
          />
        ) : (
          <OTPInput
            aria-label={t("signIn.appCode", "Code from your authenticator app")}
            value={code}
            onValueChange={onCodeChange}
            onComplete={onVerify}
            autoFocus
          />
        )}
        <Button type="submit" variant="primary" size="xl" fullWidth loading={verifying} disabled={recovery ? !code.trim() : code.length < 6}>
          {t("signIn.verify", "Verify")}
        </Button>
        <Text variant="small" tone="secondary" className="text-center">
          {tRich("signIn.challengeExpires", "The challenge expires {when}. {link}", {
            when: formatRelative(expiresAt),
            link: (
              <Link href="#" onClick={(e) => (e.preventDefault(), onToggleRecovery())}>
                {recovery ? t("signIn.useApp", "Use the authenticator app") : t("signIn.useRecovery", "Use a recovery code")}
              </Link>
            ),
          })}
        </Text>
      </form>
    </PublicPage>
  );
}
