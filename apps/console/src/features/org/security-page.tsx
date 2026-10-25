import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  AlertDialog,
  Badge,
  Button,
  Code,
  Combobox,
  Field,
  InlineCopyText,
  Input,
  NumberInput,
  RolePicker,
  SensitiveInput,
  SettingsGroup,
  SettingsRow,
  SettingsSection,
  Skeleton,
  StepDialog,
  Switch,
} from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { SecurityView, SsoView } from "~/api/generated/types";
import { orgViewKey, securityQuery } from "~/api/queries/orgs";
import { useApiMutation } from "~/auth/mutation";
import { useOrgRights, useSession } from "~/auth/session";
import { t } from "~/i18n";
import { tRich } from "~/i18n/rich";
import { common } from "~/i18n/common";
import { useCatalogue } from "~/kaeso/catalogue";
import { roleOptions, rolesOfScope, useRoles } from "~/kaeso/roles";
import { toast } from "~/shell/toast";
import { LoadFailed } from "./load-failed";
import { OrgPage } from "./org-page";
import { useOrgGiver } from "~/auth/givers";
import { useNeedsRight } from "~/auth/needs-right";

const INVITE_ONLY = "invite-only";
const looksLikeDomain = (s: string) => /^[a-z0-9-]+(\.[a-z0-9-]+)+$/.test(s);

export function OrgSecurityPage({ org }: { org: string }) {
  const security = useQuery(securityQuery(org));
  const s = security.data;
  return (
    <OrgPage
      org={org}
      title={t("org.security.title", "Security")}
      hidden={t("org.security.hidden", "You can't see the security settings here")}
      right="org.view"
      description={t("org.security.description", "How members sign in to the organization, and how long keys may live.")}
    >
      {security.isError ? (
        <LoadFailed title={t("org.security.loadFailed", "The security settings couldn't load")} />
      ) : !s ? (
        <Skeleton className="h-full w-full" />
      ) : (
        <>
          <SingleSignOn org={org} security={s} />
          <Requirements org={org} security={s} />
          <KeyPolicy org={org} days={s.max_key_lifetime_days ?? null} />
        </>
      )}
    </OrgPage>
  );
}

function SingleSignOn({ org, security }: { org: string; security: SecurityView }) {
  const needs = useNeedsRight(org);
  const rights = useOrgRights(org);
  const canManage = rights.has("security.sso.manage");
  const [editing, setEditing] = useState(false);
  const [disconnecting, setDisconnecting] = useState(false);
  const sso = security.sso;
  const verify = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/orgs/{org}/security/sso/verify", { params: { path: { org } } })),
    invalidates: [["orgs", org, "security"]],
    onSuccess: (s) => {
      if (s.sso?.domains.every((d) => d.verified)) toast.success(t("org.sso.domainsVerified", "Domains verified"));
      else toast.warning(t("org.sso.recordMissing", "No TXT record found yet; DNS changes can take a while"));
    },
  });
  const disconnect = useApiMutation({
    mutationFn: async () => {
      await api.DELETE("/platform/v1/orgs/{org}/security/sso", { params: { path: { org } } });
    },
    invalidates: [securityQuery(org).queryKey, orgViewKey(org)],
    onSuccess: () => {
      setDisconnecting(false);
      toast(t("org.sso.disconnected", "Identity provider disconnected"));
    },
    onError: () => setDisconnecting(false),
  });
  return (
    <SettingsSection
      title={t("org.sso.title", "Single sign-on")}
      description={t("org.sso.description", "Members sign in through your identity provider (OpenID Connect).")}
    >
      <SettingsGroup>
        {!sso ? (
          <SettingsRow
            title={t("org.sso.provider", "Identity provider")}
            description={t("org.sso.notConnected", "Not connected.")}
            control={
              <Button
                size="sm"
                variant="primary"
                disabled={!canManage}
                {...(canManage ? {} : { disabledReason: needs("security.sso.manage") })}
                onClick={() => setEditing(true)}
              >
                {t("org.sso.connect", "Connect an identity provider")}
              </Button>
            }
          />
        ) : (
          <>
            <SettingsRow
              title={t("org.sso.provider", "Identity provider")}
              description={sso.issuer}
              control={
                <div className="flex gap-8">
                  <Button
                    size="sm"
                    disabled={!canManage}
                    {...(canManage ? {} : { disabledReason: needs("security.sso.manage") })}
                    onClick={() => setEditing(true)}
                  >
                    {common.edit()}
                  </Button>
                  <Button
                    size="sm"
                    variant="danger-secondary"
                    disabled={!canManage}
                    {...(canManage ? {} : { disabledReason: needs("security.sso.manage") })}
                    onClick={() => setDisconnecting(true)}
                  >
                    {t("org.sso.disconnect", "Disconnect")}
                  </Button>
                </div>
              }
            />
            <SettingsRow title={t("org.sso.clientId", "Client id")} control={<Code>{sso.client_id}</Code>} />
            <SettingsRow
              title={t("org.sso.domains", "Email domains")}
              description={t("org.sso.domainsHint", "Verified domains' addresses sign in through the provider.")}
            />
            {sso.domains.map((d) => (
              <SettingsRow
                key={d.domain}
                title={
                  <span className="flex min-w-0 items-center gap-8">
                    <span className="truncate">{d.domain}</span>
                    <Badge tone={d.verified ? "success" : "warning"}>
                      {d.verified ? t("org.sso.verified", "Verified") : t("org.sso.unverified", "Unverified")}
                    </Badge>
                  </span>
                }
                description={
                  d.verified
                    ? t("org.sso.domainVerified", "Its addresses sign in through the provider.")
                    : tRich("org.sso.domainUnverified", "Add this TXT record at {name}, then verify.", {
                        name: <Code className="text-nowrap">{d.record_name}</Code>,
                      })
                }
                control={
                  d.verified ? undefined : (
                    <div className="flex min-w-0 items-center gap-8">
                      <InlineCopyText value={d.record_value} label={t("org.sso.copyRecord", "Copy TXT value")} />
                      <Button
                        size="sm"
                        disabled={!canManage}
                        {...(canManage ? {} : { disabledReason: needs("security.sso.manage") })}
                        loading={verify.isPending}
                        onClick={() => verify.mutate()}
                      >
                        {t("org.sso.verify", "Verify")}
                      </Button>
                    </div>
                  )
                }
              />
            ))}
            <SettingsRow
              title={t("org.sso.newMembers", "New members from SSO")}
              description={
                security.sso_default_role
                  ? t("org.sso.joinAsHint", "People without an invitation join as {role}.", { role: security.sso_default_role.name })
                  : t("org.sso.inviteOnlyHint", "Only invited people can join through the provider.")
              }
            />
            <SettingsRow
              title={t("org.sso.addresses", "Email addresses")}
              description={
                sso.require_email_verified
                  ? t("org.sso.addressesVerified", "Only addresses the provider marks verified sign in.")
                  : t("org.sso.addressesAny", "The provider's unverified addresses sign in too.")
              }
            />
            {sso.confirms_sign_in === false && (
              <SettingsRow
                title={t("org.sso.recentSignIn", "Recent sign-in")}
                description={
                  security.require_sso
                    ? t("org.sso.recentSignInImpossible", "SSO can't confirm one and is required, so members can't.")
                    : t("org.sso.recentSignInElsewhere", "SSO can't confirm one; members use a password or a link.")
                }
              />
            )}
            <SettingsRow title={t("org.sso.button", "Sign-in button")} description={buttonSentence(sso)} />
            <SettingsRow
              title={t("org.sso.redirectUrl", "Redirect URL")}
              description={t("org.sso.registerRedirect", "Register it at the provider as the sign-in callback.")}
              control={<InlineCopyText value={sso.redirect_url} label={t("org.sso.copyRedirect", "Copy redirect URL")} />}
            />
            <SettingsRow
              title={t("org.sso.keyId", "Key id")}
              description={t("org.sso.keyIdHint", "The master key that encrypts the client secret.")}
              control={<Code>{sso.key_id}</Code>}
            />
          </>
        )}
      </SettingsGroup>
      {editing && (
        <SsoDialog
          org={org}
          current={sso ?? null}
          defaultRole={security.sso_default_role?.id ?? null}
          redirectUrl={security.sso_redirect_url}
          onClose={() => setEditing(false)}
        />
      )}
      <AlertDialog
        open={disconnecting}
        onOpenChange={setDisconnecting}
        tone="danger"
        title={t("org.sso.disconnectConfirm", "Disconnect the identity provider?")}
        description={
          security.require_sso
            ? t("org.sso.disconnectRequiredHint", "Required single sign-on turns off too; members sign in with a password or a sign-in link again.")
            : t("org.sso.disconnectHint", "Members sign in with a password or a sign-in link again.")
        }
        confirmLabel={t("org.sso.disconnect", "Disconnect")}
        loading={disconnect.isPending}
        onConfirm={() => disconnect.mutateAsync()}
      />
    </SettingsSection>
  );
}

function buttonSentence(sso: SsoView): string {
  if (sso.name)
    return sso.auto_redirect
      ? t("org.sso.buttonNamedAuto", "Says Continue with {name} and starts on its own.", { name: sso.name })
      : t("org.sso.buttonNamed", "Says Continue with {name}.", { name: sso.name });
  return sso.auto_redirect
    ? t("org.sso.buttonPlainAuto", "Says Continue with SSO and starts on its own.")
    : t("org.sso.buttonPlain", "Says Continue with SSO.");
}

function SsoDialog({
  org,
  current,
  defaultRole,
  redirectUrl,
  onClose,
}: {
  org: string;
  current: SsoView | null;
  defaultRole: string | null;
  redirectUrl: string;
  onClose: () => void;
}) {
  const [issuer, setIssuer] = useState(current?.issuer ?? "");
  const [clientId, setClientId] = useState(current?.client_id ?? "");
  const [secret, setSecret] = useState("");
  const [domains, setDomains] = useState<string[]>(current?.domains.map((d) => d.domain) ?? []);
  const [role, setRole] = useState(defaultRole ?? INVITE_ONLY);
  const [buttonName, setButtonName] = useState(current?.name ?? "");
  const [autoRedirect, setAutoRedirect] = useState(current?.auto_redirect ?? false);
  const [requireVerified, setRequireVerified] = useState(current?.require_email_verified ?? true);
  const cat = useCatalogue(org);
  const giver = useOrgGiver(org);
  const roles = rolesOfScope(useRoles(org), "org").filter((r) => r.id !== "owner");
  const chosen = roles.find((r) => r.id === role);
  const save = useApiMutation({
    mutationFn: async () =>
      data(
        await api.PUT("/platform/v1/orgs/{org}/security/sso", {
          params: { path: { org } },
          body: {
            issuer: issuer.trim(),
            client_id: clientId.trim(),
            domains,
            default_role: role === INVITE_ONLY ? null : role,
            name: buttonName.trim(),
            auto_redirect: autoRedirect,
            require_email_verified: requireVerified,
            ...(secret ? { client_secret: secret } : {}),
          },
        }),
      ),
    invalidates: [securityQuery(org).queryKey, orgViewKey(org)],
    onSuccess: () => {
      toast.success(current ? t("org.sso.saved", "Single sign-on saved") : t("org.sso.connected", "Identity provider connected"));
      onClose();
    },
  });
  const addDomain = (raw: string) => {
    const found = raw
      .toLowerCase()
      .split(/[\s,;]+/)
      .filter(Boolean);
    const bad = found.filter((d) => !looksLikeDomain(d));
    if (bad.length)
      toast.warning(
        t("org.sso.notDomains", "{count, plural, one {{domains} isn't a domain, like example.com} other {{domains} aren't domains, like example.com}}", {
          count: bad.length,
          domains: bad.join(", "),
        }),
      );
    const good = found.filter(looksLikeDomain);
    if (good.length) setDomains((list) => [...new Set([...list, ...good])]);
  };
  const providerReason = !issuer.trim()
    ? t("org.sso.enterIssuer", "Enter the issuer URL")
    : !/^https?:\/\/\S+$/.test(issuer.trim())
      ? t("org.sso.issuerIsUrl", "The issuer is a URL, like https://login.example.com")
      : undefined;
  const clientReason = !clientId.trim()
    ? t("org.sso.enterClientId", "Enter the client id")
    : !secret && (!current || current.issuer !== issuer.trim().replace(/\/+$/, "") || current.client_id !== clientId.trim())
      ? t("org.sso.enterSecret", "Enter the client secret")
      : undefined;
  return (
    <StepDialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={current ? t("org.sso.edit", "Edit single sign-on") : t("org.sso.connect", "Connect an identity provider")}
      submitLabel={current ? common.save() : t("org.sso.connectShort", "Connect")}
      submitting={save.isPending}
      onSubmit={() => !providerReason && !clientReason && domains.length > 0 && save.mutate()}
      steps={[
        {
          id: "issuer",
          title: t("org.sso.issuerStep", "Where is your identity provider?"),
          description: t("org.sso.issuerStepHint", "Create an OpenID Connect app at your provider with this redirect URL, then enter its issuer URL."),
          blockedReason: providerReason,
          content: (
            <div className="flex flex-col gap-16">
              <Field label={t("org.sso.redirectUrl", "Redirect URL")}>
                <InlineCopyText value={redirectUrl} label={t("org.sso.copyRedirect", "Copy redirect URL")} />
              </Field>
              <Input
                label={t("org.sso.issuerUrl", "Issuer URL")}
                placeholder={t("org.sso.issuerExample", "https://login.example.com")}
                value={issuer}
                onChange={(e) => setIssuer(e.target.value)}
                autoFocus
              />
            </div>
          ),
        },
        {
          id: "client",
          title: t("org.sso.clientStep", "Which app signs people in?"),
          description: current
            ? t("org.sso.clientStepEditHint", "The app's client id; an empty secret keeps the stored one for the same issuer and app.")
            : t("org.sso.clientStepHint", "The app's client id and secret."),
          blockedReason: clientReason,
          content: (
            <div className="flex flex-col gap-12">
              <Input
                aria-label={t("org.sso.clientId", "Client id")}
                placeholder={t("org.sso.clientId", "Client id")}
                value={clientId}
                onChange={(e) => setClientId(e.target.value)}
              />
              <SensitiveInput
                aria-label={t("org.sso.clientSecret", "Client secret")}
                placeholder={t("org.sso.clientSecret", "Client secret")}
                value={secret}
                onChange={(e) => setSecret(e.target.value)}
                autoComplete="off"
              />
            </div>
          ),
        },
        {
          id: "domains",
          title: t("org.sso.domainsStep", "Which email domains sign in with it?"),
          description: t("org.sso.domainsStepHint", "Your organization's own domains, not public mail; type one and press Enter."),
          blockedReason: domains.length === 0 ? t("org.sso.addDomain", "Add at least one email domain") : undefined,
          content: (
            <Combobox
              multiple
              aria-label={t("org.sso.domains", "Email domains")}
              placeholder={t("org.sso.domainExample", "example.com")}
              options={domains.map((d) => ({ value: d, label: d }))}
              value={domains}
              onValueChange={setDomains}
              onCreate={addDomain}
              emptyText={t("org.sso.typeDomain", "Type a domain")}
            />
          ),
        },
        {
          id: "join",
          title: t("org.sso.joinStep", "Can people join without an invitation?"),
          description: role === INVITE_ONLY ? t("org.sso.inviteOnlyHint", "Only invited people can join through the provider.") : (chosen?.description ?? ""),
          content: (
            <RolePicker
              appearance="select"
              aria-label={t("org.sso.uninvited", "People without an invitation")}
              fullWidth
              value={role}
              onValueChange={setRole}
              roles={[
                {
                  value: INVITE_ONLY,
                  label: t("org.sso.inviteOnly", "Invite only"),
                  description: t("org.sso.inviteOnlyShort", "Only invited people can join"),
                },
                ...roleOptions(cat, roles, giver).map((o) => ({ ...o, label: t("org.sso.joinAs", "Join as {role}", { role: o.label }) })),
              ]}
            />
          ),
        },
        {
          id: "sign-in",
          title: t("org.sso.signInStep", "How does the sign-in page offer it?"),
          description: t("org.sso.signInStepHint", "The name on its button, and whether sign-in starts at the provider without a click."),
          content: (
            <div className="flex flex-col gap-16">
              <Input
                label={t("org.sso.buttonName", "Name on the button")}
                placeholder={t("org.sso.buttonNameExample", "Acme ID")}
                description={t("org.sso.buttonNameHint", "Leave it empty to say SSO.")}
                value={buttonName}
                onChange={(e) => setButtonName(e.target.value)}
                maxLength={50}
              />
              <Switch
                label={t("org.sso.autoRedirect", "Start without a click")}
                description={t("org.sso.autoRedirectHint", "After the email, sign-in goes straight to the provider when it's the only way in.")}
                checked={autoRedirect}
                onCheckedChange={setAutoRedirect}
              />
              <Switch
                label={t("org.sso.requireVerified", "Require verified addresses")}
                description={t(
                  "org.sso.requireVerifiedHint",
                  "Refuses tokens that don't mark the address as verified; turn off only for a provider that never does.",
                )}
                checked={requireVerified}
                onCheckedChange={setRequireVerified}
              />
            </div>
          ),
        },
      ]}
    />
  );
}

function Requirements({ org, security }: { org: string; security: SecurityView }) {
  const needs = useNeedsRight(org);
  const rights = useOrgRights(org);
  const session = useSession();
  const [confirming, setConfirming] = useState<"sso" | "2fa" | null>(null);
  const requireSso = useApiMutation({
    mutationFn: async (on: boolean) =>
      data(await api.PUT("/platform/v1/orgs/{org}/security/require-sso", { params: { path: { org } }, body: { enabled: on } })),
    invalidates: [securityQuery(org).queryKey, orgViewKey(org), ["session"]],
    onSuccess: (s) => {
      setConfirming(null);
      toast.success(s.require_sso ? t("org.sso.requiredOn", "Single sign-on is required") : t("org.sso.requiredOff", "Single sign-on is no longer required"));
    },
    onError: () => setConfirming(null),
  });
  const require2fa = useApiMutation({
    mutationFn: async (on: boolean) =>
      data(await api.PUT("/platform/v1/orgs/{org}/security/require-2fa", { params: { path: { org } }, body: { enabled: on } })),
    invalidates: [securityQuery(org).queryKey, orgViewKey(org), ["session"]],
    onSuccess: (s) => {
      setConfirming(null);
      toast.success(
        s.require_2fa
          ? t("org.twoFactor.requiredOn", "Two-factor authentication is required")
          : t("org.twoFactor.requiredOff", "Two-factor authentication is no longer required"),
      );
    },
    onError: () => setConfirming(null),
  });
  const ssoReason = !rights.has("security.sso.require")
    ? needs("security.sso.require")
    : !security.sso
      ? t("org.sso.connectFirst", "Connect an identity provider first")
      : undefined;
  const tfaReason = rights.has("security.2fa.require") ? undefined : needs("security.2fa.require");
  return (
    <SettingsSection title={t("org.security.requirements", "Requirements")}>
      <SettingsGroup>
        <SettingsRow
          title={t("org.sso.require", "Require single sign-on")}
          description={
            security.sso
              ? t("org.sso.requireHint", "Accounts at its domains use only SSO; guests keep their own.")
              : t("org.sso.connectFirst", "Connect an identity provider first")
          }
          control={
            <Switch
              aria-label={t("org.sso.require", "Require single sign-on")}
              checked={security.require_sso}
              disabled={!!ssoReason}
              {...(ssoReason ? { disabledReason: ssoReason } : {})}
              loading={requireSso.isPending}
              onCheckedChange={(on) => (on ? setConfirming("sso") : requireSso.mutate(false))}
            />
          }
        />
        <SettingsRow
          title={t("org.twoFactor.require", "Require two-factor authentication")}
          description={t("org.twoFactor.requireHint", "Needs a code at sign-in; single sign-on counts as one.")}
          control={
            <Switch
              aria-label={t("org.twoFactor.require", "Require two-factor authentication")}
              checked={security.require_2fa}
              disabled={!!tfaReason}
              {...(tfaReason ? { disabledReason: tfaReason } : {})}
              loading={require2fa.isPending}
              onCheckedChange={(on) => (on && !session.totp_enabled ? setConfirming("2fa") : require2fa.mutate(on))}
            />
          }
        />
      </SettingsGroup>
      <AlertDialog
        open={confirming === "sso"}
        onOpenChange={(o) => !o && setConfirming(null)}
        tone="warning"
        title={t("org.sso.requireConfirm", "Require single sign-on?")}
        description={t("org.sso.requireConfirmHint", "Accounts at its verified domains then sign in only through SSO, and their other sessions end.")}
        confirmLabel={t("org.sso.require", "Require single sign-on")}
        loading={requireSso.isPending}
        onConfirm={() => requireSso.mutateAsync(true)}
      />
      <AlertDialog
        open={confirming === "2fa"}
        onOpenChange={(o) => !o && setConfirming(null)}
        tone="warning"
        title={t("org.twoFactor.requireConfirm", "Require two-factor authentication?")}
        description={t("org.twoFactor.requireConfirmHint", "You don't use it yet; the organization closes to you and members without it until you set it up.")}
        confirmLabel={t("org.twoFactor.require", "Require two-factor authentication")}
        loading={require2fa.isPending}
        onConfirm={() => require2fa.mutateAsync(true)}
      />
    </SettingsSection>
  );
}

function KeyPolicy({ org, days }: { org: string; days: number | null }) {
  const needs = useNeedsRight(org);
  const rights = useOrgRights(org);
  const canManage = rights.has("security.key_policy.manage");
  const [draft, setDraft] = useState(days === null ? "" : String(days));
  const save = useApiMutation({
    mutationFn: async (next: number | null) =>
      data(await api.PUT("/platform/v1/orgs/{org}/security/key-policy", { params: { path: { org } }, body: { max_key_lifetime_days: next } })),
    invalidates: [securityQuery(org).queryKey, orgViewKey(org)],
    onSuccess: (s) => {
      setDraft(s.max_key_lifetime_days == null ? "" : String(s.max_key_lifetime_days));
      toast.success(t("org.keyLifetime.saved", "Key lifetime saved"));
    },
  });
  const value = Number(draft);
  const problem = !draft
    ? t("org.keyLifetime.enterDays", "Enter a number of days")
    : !Number.isInteger(value) || value < 1
      ? t("org.wholeDays", "Enter whole days, at least 1")
      : value === days
        ? t("org.noChange", "No change to save")
        : undefined;
  return (
    <SettingsSection
      title={t("org.keyLifetime.title", "Maximum key lifetime")}
      description={t("org.keyLifetime.description", 'Existing keys keep their expiry; new keys lose the "Never" option.')}
    >
      <SettingsGroup>
        <SettingsRow
          title={t("org.keyLifetime.noLimit", "No limit")}
          description={
            days === null
              ? t("org.keyLifetime.never", "Keys may never expire.")
              : t("org.keyLifetime.within", "New keys expire within {days} at most.", {
                  days: t("org.days", "{count, plural, one {# day} other {# days}}", { count: days }),
                })
          }
          control={
            <Switch
              aria-label={t("org.keyLifetime.noLimit", "No limit")}
              checked={days === null}
              disabled={!canManage}
              {...(canManage ? {} : { disabledReason: needs("security.key_policy.manage") })}
              loading={save.isPending}
              onCheckedChange={(on) => save.mutate(on ? null : 90)}
            />
          }
        />
        {days !== null && (
          <SettingsRow
            title={t("org.keyLifetime.max", "Maximum lifetime")}
            description={t("org.keyLifetime.maxHint", "Projects can set a shorter one.")}
            control={
              <div className="flex min-w-0 items-center gap-8">
                <div className="w-160 min-w-0">
                  <NumberInput
                    aria-label={t("org.keyLifetime.title", "Maximum key lifetime")}
                    size="sm"
                    integer
                    value={draft}
                    onValueChange={setDraft}
                    min="1"
                    step="1"
                    unit={t("org.daysUnit", "days")}
                    disabled={!canManage}
                  />
                </div>
                <Button
                  size="sm"
                  disabled={!canManage || !!problem}
                  {...(problem && canManage ? { disabledReason: problem } : {})}
                  loading={save.isPending}
                  onClick={() => save.mutate(value)}
                >
                  {common.save()}
                </Button>
              </div>
            }
          />
        )}
      </SettingsGroup>
    </SettingsSection>
  );
}
