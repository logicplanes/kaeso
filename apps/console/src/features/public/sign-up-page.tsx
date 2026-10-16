import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Button, Input, Link, SensitiveInput, Text } from "@logicplanes/kaeso-ui";
import { PublicPage } from "~/shell/public-page";
import { api, data } from "~/api/client";
import { takeFragment } from "~/auth/fragment";
import { usePageTitle } from "~/shell/title";
import { signInMethodsQuery } from "~/auth/sign-in-api";
import { passwordHint, passwordOk, useSubmit } from "./form";
import { t } from "~/i18n";
import { tRich } from "~/i18n/rich";

export function SignUpPage() {
  usePageTitle(t("public.createAnAccount", "Create an account"));
  const [email, setEmail] = useState("");
  const [sent, setSent] = useState<string | null>(null);
  const install = useQuery(signInMethodsQuery);
  const { busy, submit } = useSubmit();
  if (install.data && !install.data.sign_up) {
    return (
      <PublicPage
        title={t("public.byInvitation", "Accounts here are by invitation")}
        description={t("public.byInvitationHint", "Ask an administrator to invite you.")}
      >
        <Button size="xl" fullWidth href="/sign-in">
          {t("public.backToSignIn", "Back to sign in")}
        </Button>
      </PublicPage>
    );
  }
  if (sent) {
    return (
      <PublicPage title={t("public.checkEmail", "Check your email")} description={sent}>
        <Button size="xl" fullWidth href="/sign-in">
          {t("public.backToSignIn", "Back to sign in")}
        </Button>
      </PublicPage>
    );
  }
  return (
    <PublicPage
      title={t("public.createAnAccount", "Create an account")}
      description={t("public.createAnAccountHint", "We email you a link to finish your account.")}
    >
      <form
        className="flex flex-col gap-16"
        onSubmit={(e) => {
          e.preventDefault();
          void submit(async () => {
            setSent(data(await api.POST("/platform/v1/auth/signup", { body: { email: email.trim() } })).message);
          });
        }}
      >
        <Input
          label={t("common.email", "Email")}
          size="xl"
          type="email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          autoComplete="email"
          autoFocus
        />
        <Button type="submit" variant="primary" size="xl" fullWidth loading={busy} disabled={!email.trim()}>
          {t("common.continue", "Continue")}
        </Button>
        <Text variant="small" tone="secondary" className="text-center">
          {tRich("public.haveAccount", "Have an account? {link}", { link: <Link href="/sign-in">{t("public.signIn", "Sign in")}</Link> })}
        </Text>
      </form>
    </PublicPage>
  );
}

export function SignUpVerifyPage() {
  usePageTitle(t("public.finishAccount", "Finish your account"));
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [token] = useState(() => takeFragment("token"));
  const [name, setName] = useState("");
  const [password, setPassword] = useState("");
  const { busy, submit } = useSubmit();
  const ready = !!token && name.trim() && (password === "" || passwordOk(password));
  if (!token) {
    return (
      <PublicPage
        title={t("public.needsSignUpLink", "This page needs the sign-up link")}
        description={t("public.needsSignUpLinkHint", "Open the link from the sign-up email; it ends with #token=…")}
      >
        <Button size="xl" fullWidth href="/sign-up">
          {t("public.startAgain", "Start again")}
        </Button>
      </PublicPage>
    );
  }
  return (
    <PublicPage
      title={t("public.finishAccount", "Finish your account")}
      description={t("public.finishAccountHint", "Choose the name people see; a password is optional.")}
    >
      <form
        className="flex flex-col gap-16"
        onSubmit={(e) => {
          e.preventDefault();
          if (!ready || !token) return;
          void submit(async () => {
            await api.POST("/platform/v1/auth/signup/verify", { body: { token, name: name.trim(), ...(password ? { password } : {}) } });
            await queryClient.resetQueries();
            await navigate({ to: "/", replace: true });
          });
        }}
      >
        <Input
          label={t("common.name", "Name")}
          size="xl"
          value={name}
          onChange={(e) => setName(e.target.value)}
          autoComplete="name"
          autoFocus
          maxLength={100}
        />
        <SensitiveInput
          label={t("public.password", "Password")}
          optional
          kind="password"
          size="xl"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          autoComplete="new-password"
          description={password ? passwordHint(password) : t("public.passwordOptionalHint", "Leave empty to sign in with a link by email.")}
        />
        <Button type="submit" variant="primary" size="xl" fullWidth loading={busy} disabled={!ready}>
          {t("public.createAccount", "Create account")}
        </Button>
      </form>
    </PublicPage>
  );
}
