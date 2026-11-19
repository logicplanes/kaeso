import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Badge, Button, CodeBlock, CopyButton, Input, Link, Section, SettingsGroup, SettingsRow, Switch, Text } from "@logicplanes/kaeso-ui";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import { fieldError } from "~/api/field-error";
import { isApiError } from "~/api/errors";
import type { LinkedBillingView } from "~/api/generated/types";
import { linkedBillingQuery } from "~/api/queries/instance";
import { useApiMutation } from "~/auth/mutation";
import { t } from "~/i18n";
import { SettingsLoading } from "~/shell/settings-page";
import { toast } from "~/shell/toast";
import { LoadFailed } from "~/features/org/load-failed";
import { InstancePage } from "./instance-page";

export function InstanceLinkedBillingPage() {
  const offers = useQuery(linkedBillingQuery);
  return (
    <InstancePage
      title={t("shell.settings.linkedBilling", "Linked billing")}
      description={t("instance.linkedBillingHint", "Organizations pay providers through their own organization there, without a key.")}
    >
      {offers.isError ? (
        <LoadFailed title={t("instance.linkedBillingFailed", "Linked billing couldn't load")} />
      ) : !offers.data ? (
        <SettingsLoading rows={4} />
      ) : (
        offers.data.map((o) => <Offer key={o.provider} offer={o} />)
      )}
    </InstancePage>
  );
}

function problemText(problem: string, provider: string): string {
  switch (problem) {
    case "federation":
      return t("instance.linkedProblemFederation", "{provider} refused the service account token; check the issuer and the rule.", { provider });
    case "client":
      return t("instance.linkedProblemClient", "{provider} refused the client; check the client ID, audience and issuer.", { provider });
    case "scope":
      return t("instance.linkedProblemScope", "The app lacks billing:attribution or Offline access.");
    case "unauthorized":
      return t("instance.linkedProblemUnauthorized", "The app has no operator service account or workspace.");
    case "unreachable":
      return t("instance.linkedProblemUnreachable", "{provider} couldn't be reached; try again shortly.", { provider });
    default:
      return t("instance.linkedProblemRequest", "{provider} refused the link request; check the redirect URI.", { provider });
  }
}

function Offer({ offer }: { offer: LinkedBillingView }) {
  const app = offer.app;
  const [editing, setEditing] = useState(!app);
  const check = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/instance/linked-billing/{provider}/check", { params: { path: { provider: offer.provider } } })),
    invalidates: [["instance", "linked-billing"]],
    onSuccess: (v) =>
      v.app?.check_problem ? toast.error(problemText(v.app.check_problem, offer.provider_name)) : toast.success(t("instance.linkedChecked", "The app works")),
  });
  const remove = useApiMutation({
    mutationFn: async () => {
      await api.DELETE("/platform/v1/instance/linked-billing/{provider}", { params: { path: { provider: offer.provider } } });
    },
    invalidates: [["instance", "linked-billing"]],
    onSuccess: () => toast(t("instance.linkedRemoved", "OAuth app removed")),
  });
  return (
    <>
      <Section
        spacing="settings"
        title={t("instance.linkedRegister", "Register the app at {provider}", { provider: offer.provider_name })}
        description={t("instance.linkedRegisterHint", "Create an OAuth app in {where} with these values.", { where: new URL(offer.apps_url).host })}
        actions={
          <Link href={offer.docs_url} external>
            {t("instance.linkedGuide", "Setup guide")}
          </Link>
        }
      >
        <SettingsGroup>
          <SettingsRow
            title={t("instance.linkedRedirect", "Redirect URI")}
            description={t("instance.linkedRedirectHint", "From the install's public URL; register it exactly.")}
            control={<CopyButton display="inline" value={offer.redirect_uri} label={t("instance.copyRedirect", "Copy redirect URI")} />}
          />
          <SettingsRow
            title={t("instance.linkedIssuer", "Issuer")}
            description={t("instance.linkedIssuerHint", "Register it as the app's identity provider.")}
            control={<CopyButton display="inline" value={offer.issuer} label={t("instance.copyIssuer", "Copy issuer")} />}
          />
          <SettingsRow
            title={t("instance.linkedJwks", "JWKS URL")}
            description={t("instance.linkedJwksHint", "Where the issuer's public keys are.")}
            control={<CopyButton display="inline" value={offer.jwks_url} label={t("instance.copyJwks", "Copy JWKS URL")} />}
          />
          <SettingsRow
            title={t("instance.linkedSubject", "Subject")}
            description={t("instance.linkedSubjectHint", "The subject pattern that client assertions match.")}
            control={<CopyButton display="inline" value={offer.subject} label={t("instance.copySubject", "Copy subject")} />}
          />
        </SettingsGroup>
        <div className="mt-12">
          <CodeBlock
            title={t("instance.linkedKeySet", "Key set, for an install the provider can't reach")}
            code={JSON.stringify(offer.jwks, null, 2)}
            lang="json"
            copy
            collapseAfter={6}
          />
        </div>
      </Section>
      <Section
        spacing="settings"
        title={t("instance.linkedApp", "The install's OAuth app")}
        description={
          app
            ? t("instance.linkedAppUsed", "{count, plural, one {# linked key uses it} other {# linked keys use it}}.", { count: offer.linked_keys })
            : t("instance.linkedAppMissing", "Until it is set, organizations can't link.")
        }
        actions={
          app && !editing ? (
            <span className="flex items-center gap-8">
              <Button size="sm" loading={check.isPending} onClick={() => check.mutate()}>
                {t("instance.linkedCheck", "Check")}
              </Button>
              <Button size="sm" onClick={() => setEditing(true)}>
                {t("instance.linkedReplace", "Replace")}
              </Button>
              <Button size="sm" variant="danger" loading={remove.isPending} onClick={() => remove.mutate()}>
                {t("instance.linkedRemove", "Remove")}
              </Button>
            </span>
          ) : undefined
        }
      >
        {app && !editing ? (
          <SettingsGroup>
            <SettingsRow title={t("instance.linkedClientId", "Client ID")} control={<CopyButton display="inline" value={app.client_id} />} />
            <SettingsRow title={t("instance.linkedRule", "Federation rule")} control={<Text tone="secondary">{app.federation_rule_id}</Text>} />
            <SettingsRow
              title={t("instance.linkedLastCheck", "Last check")}
              description={app.check_problem ? problemText(app.check_problem, offer.provider_name) : undefined}
              control={
                app.checked_at ? (
                  app.check_problem ? (
                    <Badge tone="warning">{t("instance.linkedCheckFailed", "Failed")}</Badge>
                  ) : (
                    <Text tone="secondary">{formatDate(app.checked_at, { time: true })}</Text>
                  )
                ) : (
                  <Text tone="tertiary">{t("instance.linkedNotChecked", "Not checked yet")}</Text>
                )
              }
            />
          </SettingsGroup>
        ) : (
          <AppForm offer={offer} onDone={() => setEditing(false)} />
        )}
      </Section>
    </>
  );
}

function AppForm({ offer, onDone }: { offer: LinkedBillingView; onDone: () => void }) {
  const app = offer.app;
  const [clientId, setClientId] = useState(app?.client_id ?? "");
  const [audience, setAudience] = useState(app?.audience ?? "");
  const [organization, setOrganization] = useState(app?.organization_id ?? "");
  const [rule, setRule] = useState(app?.federation_rule_id ?? "");
  const [workspace, setWorkspace] = useState(app?.workspace_id ?? "");
  const [serviceAccount, setServiceAccount] = useState(app?.service_account_id ?? "");
  const [policy, setPolicy] = useState(app?.send_billing_policy ?? false);
  const save = useApiMutation({
    mutationFn: async () =>
      data(
        await api.PUT("/platform/v1/instance/linked-billing/{provider}", {
          params: { path: { provider: offer.provider } },
          body: {
            client_id: clientId.trim(),
            audience: audience.trim(),
            organization_id: organization.trim(),
            federation_rule_id: rule.trim(),
            ...(workspace.trim() ? { workspace_id: workspace.trim() } : {}),
            ...(serviceAccount.trim() ? { service_account_id: serviceAccount.trim() } : {}),
            send_billing_policy: policy,
          },
        }),
      ),
    invalidates: [["instance", "linked-billing"]],
    onSuccess: () => {
      toast.success(t("instance.linkedSaved", "OAuth app saved"));
      onDone();
    },
  });
  const error = isApiError(save.error) ? save.error : null;
  const missing = !clientId.trim() || !audience.trim() || !organization.trim() || !rule.trim();
  return (
    <div className="flex flex-col gap-12">
      <Input
        label={t("instance.linkedClientId", "Client ID")}
        value={clientId}
        onChange={(e) => setClientId(e.target.value)}
        error={fieldError(error, "client_id")}
      />
      <Input
        label={t("instance.linkedAudience", "Audience")}
        description={t("instance.linkedAudienceHint", "As the app's Client authentication tab shows it.")}
        value={audience}
        onChange={(e) => setAudience(e.target.value)}
        error={fieldError(error, "audience")}
      />
      <Input
        label={t("instance.linkedOrganization", "Organization ID")}
        description={t("instance.linkedOrganizationHint", "The UUID of the install's own organization at {provider}.", { provider: offer.provider_name })}
        value={organization}
        onChange={(e) => setOrganization(e.target.value)}
        error={fieldError(error, "organization_id")}
      />
      <Input
        label={t("instance.linkedRule", "Federation rule")}
        description={t("instance.linkedRuleHint", "The rule that maps the issuer to the operator service account.")}
        value={rule}
        onChange={(e) => setRule(e.target.value)}
        error={fieldError(error, "federation_rule_id")}
      />
      <Input
        label={t("instance.linkedWorkspace", "Workspace ID")}
        description={t("instance.linkedWorkspaceHint", "Only when the rule covers more than one workspace.")}
        value={workspace}
        onChange={(e) => setWorkspace(e.target.value)}
        error={fieldError(error, "workspace_id")}
      />
      <Input
        label={t("instance.linkedServiceAccount", "Service account ID")}
        value={serviceAccount}
        onChange={(e) => setServiceAccount(e.target.value)}
        error={fieldError(error, "service_account_id")}
      />
      <Switch
        label={t("instance.linkedPolicy", "Name the linked profile on each request")}
        description={t("instance.linkedPolicyHint", "Turn on only if {provider} says your app needs billing_policy.", { provider: offer.provider_name })}
        checked={policy}
        onCheckedChange={setPolicy}
      />
      <div className="flex gap-8">
        <Button
          variant="primary"
          loading={save.isPending}
          disabled={missing}
          disabledReason={missing ? t("instance.linkedMissing", "Fill in the client ID, audience, organization and rule") : undefined}
          onClick={() => save.mutate()}
        >
          {t("common.save", "Save")}
        </Button>
        {app && <Button onClick={onDone}>{t("common.cancel", "Cancel")}</Button>}
      </div>
    </div>
  );
}
