import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Button, Code, Input, SensitiveInput, Text } from "@logicplanes/kaeso-ui";
import { PublicPage } from "~/shell/public-page";
import { api } from "~/api/client";
import { takeFragment } from "~/auth/fragment";
import { sessionQuery } from "~/auth/session";
import { signInMethodsQuery } from "~/auth/sign-in-api";
import { usePageTitle } from "~/shell/title";
import { fieldError } from "~/api/field-error";
import { passwordHint, passwordOk, useSubmit } from "./form";
import { t } from "~/i18n";
import { tRich } from "~/i18n/rich";

const SETUP_COMMAND = "kaeso setup-link";
const SIGN_IN_LINK_COMMAND = "kaeso user sign-in-link";

export function SetupPage() {
  usePageTitle(t("public.setUpKaeso", "Set up Kaeso"));
  const [token] = useState(() => takeFragment("token"));
  const install = useQuery(signInMethodsQuery);
  if (install.data?.set_up) return <AlreadySetUp />;
  return token ? <SetupForm token={token} /> : <NeedsSetupLink unusable={false} />;
}

function AlreadySetUp() {
  return (
    <PublicPage
      title={t("public.alreadySetUp", "Kaeso is set up")}
      description={t("public.alreadySetUpHint", "Sign in with your account; a setup link works only until the first user exists.")}
    >
      <Button variant="primary" size="xl" fullWidth href="/sign-in">
        {t("public.signIn", "Sign in")}
      </Button>
    </PublicPage>
  );
}

function NeedsSetupLink({ unusable }: { unusable: boolean }) {
  return (
    <PublicPage
      title={unusable ? t("public.setupLinkUnusable", "This setup link can't be used") : t("public.needsSetupLink", "This page needs the setup link")}
      description={t("public.setupLinkFromCommand", "The gateway's command line prints the setup link; it ends with #token=…")}
    >
      <Text tone="secondary" className="text-center">
        {tRich("public.runSetupLinkHere", "Run {command} where the gateway runs to print a new link.", { command: <Code>{SETUP_COMMAND}</Code> })}
      </Text>
    </PublicPage>
  );
}

function SetupForm({ token }: { token: string }) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const install = useQuery(signInMethodsQuery);
  const withPassword = install.data?.password ?? true;
  const ssoOnly = install.data ? install.data.sso && !install.data.password && !install.data.magic_link : false;
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [orgName, setOrgName] = useState("");
  const { busy, error, submit } = useSubmit();
  const ready = name.trim() && email.trim() && (!withPassword || passwordOk(password)) && orgName.trim();
  const tokenProblem = error?.param === "token";

  const setUp = () =>
    submit(async () => {
      await api.POST("/platform/v1/auth/setup", {
        body: { token, name: name.trim(), email: email.trim(), ...(withPassword ? { password } : {}), org_name: orgName.trim() },
      });
      await queryClient.resetQueries();
      const org = ssoOnly ? (await queryClient.fetchQuery(sessionQuery)).memberships[0]?.org_id : undefined;
      await (org ? navigate({ to: "/settings/orgs/$org/security", params: { org } }) : navigate({ to: "/" }));
    });

  if (error?.type === "conflict") return <AlreadySetUp />;
  if (tokenProblem) return <NeedsSetupLink unusable />;
  return (
    <PublicPage
      title={t("public.setUpKaeso", "Set up Kaeso")}
      description={
        withPassword
          ? t("public.setUpKaesoHint", "Create the first user, an instance admin, and the organization they own.")
          : t("public.setUpWithoutPassword", "Create the first user, an instance admin, and the organization they own; passwords are off here.")
      }
    >
      <form className="flex flex-col gap-16" onSubmit={(e) => (e.preventDefault(), ready && void setUp())}>
        <Input
          label={t("common.name", "Name")}
          size="xl"
          value={name}
          onChange={(e) => setName(e.target.value)}
          autoComplete="name"
          autoFocus
          maxLength={100}
          error={fieldError(error, "name")}
        />
        <Input
          label={t("common.email", "Email")}
          size="xl"
          type="email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          autoComplete="username"
          error={fieldError(error, "email")}
        />
        {withPassword && (
          <SensitiveInput
            label={t("public.password", "Password")}
            kind="password"
            size="xl"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            autoComplete="new-password"
            description={passwordHint(password)}
            error={fieldError(error, "password")}
          />
        )}
        <Input
          label={t("public.orgName", "Organization name")}
          size="xl"
          value={orgName}
          onChange={(e) => setOrgName(e.target.value)}
          autoComplete="organization"
          maxLength={100}
          error={fieldError(error, "org_name")}
        />
        <Button type="submit" variant="primary" size="xl" fullWidth loading={busy} disabled={!ready}>
          {t("public.setUpKaeso", "Set up Kaeso")}
        </Button>
        <Text variant="small" tone="secondary" className="text-center">
          {ssoOnly
            ? tRich("public.setUpSsoNext", "Next, connect your identity provider. Until it works, {command} prints a sign-in link for you.", {
                command: <Code>{SIGN_IN_LINK_COMMAND}</Code>,
              })
            : t("public.setUpAfterwards", "You can invite more people and add organizations afterwards.")}
        </Text>
      </form>
    </PublicPage>
  );
}
