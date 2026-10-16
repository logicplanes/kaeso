import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Button, Code, Input, SensitiveInput, Text } from "@logicplanes/kaeso-ui";
import { PublicPage } from "~/shell/public-page";
import { api } from "~/api/client";
import { takeFragment } from "~/auth/fragment";
import { usePageTitle } from "~/shell/title";
import { fieldError } from "~/api/field-error";
import { passwordHint, passwordOk, useSubmit } from "./form";
import { t } from "~/i18n";
import { tRich } from "~/i18n/rich";

const SETUP_COMMAND = "kaeso setup-link";

export function SetupPage() {
  usePageTitle(t("public.setUpKaeso", "Set up Kaeso"));
  const [token] = useState(() => takeFragment("token"));
  return token ? <SetupForm token={token} /> : <NeedsSetupLink unusable={false} />;
}

function NeedsSetupLink({ unusable }: { unusable: boolean }) {
  return (
    <PublicPage
      title={unusable ? t("public.setupLinkUnusable", "This setup link can't be used") : t("public.needsSetupLink", "This page needs the setup link")}
      description={t("public.needsSetupLinkHint", "Open the setup link from the gateway's log; it ends with #token=…")}
    >
      <Text tone="secondary">
        {tRich("public.runSetupLink", "Run {command} on the gateway to print a new link.", { command: <Code>{SETUP_COMMAND}</Code> })}
      </Text>
    </PublicPage>
  );
}

function SetupForm({ token }: { token: string }) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [orgName, setOrgName] = useState("");
  const { busy, error, submit } = useSubmit();
  const ready = name.trim() && email.trim() && passwordOk(password) && orgName.trim();
  const tokenProblem = error?.param === "token" || error?.type === "conflict";

  const setUp = () =>
    submit(async () => {
      await api.POST("/platform/v1/auth/setup", { body: { token, name: name.trim(), email: email.trim(), password, org_name: orgName.trim() } });
      await queryClient.resetQueries();
      await navigate({ to: "/" });
    });

  if (tokenProblem) return <NeedsSetupLink unusable />;
  return (
    <PublicPage
      title={t("public.setUpKaeso", "Set up Kaeso")}
      description={t("public.setUpKaesoHint", "Create the first user, an instance admin, and the organization they own.")}
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
          {t("public.setUpAfterwards", "You can invite more people and add organizations afterwards.")}
        </Text>
      </form>
    </PublicPage>
  );
}
