import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Button, Input, Link, SensitiveInput, Text } from "@logicplanes/kaeso-ui";
import { PublicPage } from "~/shell/public-page";
import { api, data } from "~/api/client";
import { isApiError } from "~/api/errors";
import { forgetFragment, takeFragment } from "~/auth/fragment";
import { usePageTitle } from "~/shell/title";
import { fieldError } from "~/api/field-error";
import { passwordHint, passwordOk, useSubmit } from "./form";
import { takeResetAddress } from "./reset-address";
import { t } from "~/i18n";

export function PasswordResetPage() {
  const [token] = useState(() => takeFragment("token"));
  return token ? <NewPassword key={token} token={token} /> : <RequestReset />;
}

function RequestReset() {
  usePageTitle(t("public.resetPassword", "Reset your password"));
  const [email, setEmail] = useState(takeResetAddress);
  const [sent, setSent] = useState<string | null>(null);
  const { busy, error, submit } = useSubmit();
  if (sent) {
    return (
      <PublicPage
        title={t("public.checkEmail", "Check your email")}
        description={t("public.resetSent", "If an account exists for {email}, a reset link is on its way.", { email: sent })}
      >
        <Button size="xl" fullWidth href="/sign-in">
          {t("public.backToSignIn", "Back to sign in")}
        </Button>
      </PublicPage>
    );
  }
  return (
    <PublicPage
      title={t("public.resetPassword", "Reset your password")}
      description={t("public.resetPasswordHint", "We email you a link to set a new one, valid for 15 minutes.")}
    >
      <form
        className="flex flex-col gap-16"
        onSubmit={(e) => {
          e.preventDefault();
          void submit(async () => {
            await api.POST("/platform/v1/auth/password-reset", { body: { email: email.trim() } });
            setSent(email.trim());
          });
        }}
      >
        <Input
          label={t("common.email", "Email")}
          size="xl"
          type="email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          autoComplete="username"
          autoFocus
          error={fieldError(error, "email")}
        />
        <Button type="submit" variant="primary" size="xl" fullWidth loading={busy} disabled={!email.trim()}>
          {t("public.emailMeLink", "Email me a link")}
        </Button>
        <Text variant="small" tone="secondary" className="text-center">
          <Link href="/sign-in">{t("public.backToSignIn", "Back to sign in")}</Link>
        </Text>
      </form>
    </PublicPage>
  );
}

function NewPassword({ token }: { token: string }) {
  usePageTitle(t("public.newPasswordTitle", "Set a new password"));
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [password, setPassword] = useState("");
  const { busy, error, submit } = useSubmit();
  const account = useQuery({
    queryKey: ["password-reset", token],
    queryFn: async () => data(await api.POST("/platform/v1/auth/password-reset/lookup", { body: { token } })),
    retry: false,
    staleTime: Infinity,
    meta: { quiet: true },
  });
  const expired = error?.type === "authentication" || (isApiError(account.error) && account.error.type === "authentication");
  if (expired) {
    return (
      <PublicPage
        title={t("public.linkUsed", "This link has expired or was already used.")}
        description={t("public.resetLinkOnce", "A reset link works once, for 15 minutes.")}
      >
        <Button variant="primary" size="xl" fullWidth href="/auth/password-reset">
          {t("public.requestNewLink", "Request a new link")}
        </Button>
      </PublicPage>
    );
  }
  return (
    <PublicPage
      title={t("public.newPasswordTitle", "Set a new password")}
      description={t("public.newPasswordHint", "Every session of your account ends; sign in again with the new one.")}
    >
      <form
        className="flex flex-col gap-16"
        onSubmit={(e) => {
          e.preventDefault();
          if (!passwordOk(password)) return;
          void submit(async () => {
            await api.POST("/platform/v1/auth/password-reset/verify", { body: { token, password } });
            forgetFragment(token);
            await navigate({ to: "/sign-in", search: { done: "password-changed" }, replace: true });
            queryClient.removeQueries({ queryKey: ["password-reset", token], exact: true });
          });
        }}
      >
        {account.data && <input type="email" name="username" autoComplete="username" value={account.data.email} readOnly hidden />}
        <SensitiveInput
          label={t("public.newPassword", "New password")}
          kind="password"
          size="xl"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          autoComplete="new-password"
          autoFocus
          description={passwordHint(password)}
          error={fieldError(error, "password")}
        />
        <Button type="submit" variant="primary" size="xl" fullWidth loading={busy} disabled={!passwordOk(password)}>
          {t("public.setPassword", "Set password")}
        </Button>
      </form>
    </PublicPage>
  );
}
