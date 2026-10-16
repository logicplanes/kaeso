import { useState } from "react";
import { Button, Dialog, Select, SettingsGroup, SettingsRow, SettingsSection, TagInput } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { AccountCreation, OrgCreation, SettingsView, SettingsUpdate } from "~/api/generated/types";
import { useApiMutation } from "~/auth/mutation";
import { toast } from "~/shell/toast";
import { t } from "~/i18n";

const accountOptions = (): { value: AccountCreation; label: string; description: string }[] => [
  {
    value: "invitation_only",
    label: t("instance.invitationOnly", "Invitation only"),
    description: t("instance.invitationOnlyHint", "Only invited people can create an account."),
  },
  {
    value: "allowed_domains",
    label: t("instance.allowedEmailDomains", "Allowed email domains"),
    description: t("instance.allowedEmailDomainsHint", "Addresses on the allowed domains can sign up."),
  },
  { value: "everyone", label: t("instance.anyone", "Anyone"), description: t("instance.anyoneHint", "Anyone with an email address can sign up.") },
];

const orgOptions = (): { value: OrgCreation; label: string; description: string }[] => [
  {
    value: "instance_admins",
    label: t("instance.adminsOnlyOption", "Instance admins only"),
    description: t("instance.adminsOnlyOptionHint", "Instance admins create organizations and appoint owners."),
  },
  {
    value: "allowed_domains",
    label: t("instance.usersOnDomains", "Users on allowed domains"),
    description: t("instance.usersOnDomainsHint", "Signed-in users with an address on the allowed domains."),
  },
  {
    value: "everyone",
    label: t("instance.anySignedInUser", "Any signed-in user"),
    description: t("instance.anySignedInUserHint", "Whoever creates one becomes its owner."),
  },
];

const accountSaved = (v: AccountCreation) =>
  v === "invitation_only"
    ? t("instance.accountsByInvitation", "Accounts are by invitation only")
    : v === "allowed_domains"
      ? t("instance.domainsCanSignUp", "Allowed email domains can sign up")
      : t("instance.anyoneCanSignUp", "Anyone can create an account");

const orgSaved = (v: OrgCreation) =>
  v === "instance_admins"
    ? t("instance.onlyAdminsCreateOrgs", "Only instance admins can create organizations")
    : v === "allowed_domains"
      ? t("instance.domainsCreateOrgs", "Allowed email domains can create organizations")
      : t("instance.anyoneCreatesOrgs", "Any signed-in user can create organizations");

const EXAMPLE_DOMAIN = "example.com";

const looksLikeDomain = (tag: string) => /^(\*\.|@)?[\p{L}\p{N}-]+(\.[\p{L}\p{N}-]+)+\.?$/u.test(tag.trim());

function domainSummary(domains: string[]): string {
  const shown: string[] = [];
  for (const d of domains) {
    if (shown.length > 0 && [...shown, d].join(", ").length > 40) break;
    shown.push(d);
  }
  const text = shown.join(", ");
  const rest = domains.length - shown.length;
  if (text.length > 48) return t("instance.domainCount", "{count, plural, one {# domain} other {# domains}}", { count: domains.length });
  return rest > 0 ? t("instance.domainsAndMore", "{domains} and {count} more", { domains: text, count: rest }) : text;
}

type Pending = { account_creation: AccountCreation } | { org_creation: OrgCreation } | null;

export function AccountsSettings({ settings }: { settings: SettingsView }) {
  const [editing, setEditing] = useState<{ pending: Pending } | null>(null);
  const save = useApiMutation({
    mutationFn: async (body: SettingsUpdate) => data(await api.PATCH("/platform/v1/instance/settings", { body })),
    invalidates: [["instance", "settings"], ["session"]],
    onSuccess: (s, body) => {
      setEditing(null);
      toast.success(
        body.account_creation
          ? accountSaved(s.account_creation)
          : body.org_creation
            ? orgSaved(s.org_creation)
            : t("instance.domainsSaved", "Allowed domains saved"),
      );
    },
  });
  const choose = (change: { account_creation: AccountCreation } | { org_creation: OrgCreation }) => {
    const uses = "account_creation" in change ? change.account_creation === "allowed_domains" : change.org_creation === "allowed_domains";
    if (uses && settings.allowed_domains.length === 0) setEditing({ pending: change });
    else save.mutate(change);
  };
  const usesDomains = settings.account_creation === "allowed_domains" || settings.org_creation === "allowed_domains";
  return (
    <SettingsSection
      title={t("instance.accounts", "Accounts")}
      description={t("instance.accountsHint", "Who can create an account, and who can create organizations.")}
    >
      <SettingsGroup>
        <SettingsRow
          title={t("instance.accountCreation", "Account creation")}
          description={t("instance.accountCreationHint", "Invited people can always create their account.")}
          control={
            <div className="w-240">
              <Select
                aria-label={t("instance.accountCreation", "Account creation")}
                size="sm"
                fullWidth
                value={settings.account_creation}
                disabled={save.isPending}
                onValueChange={(v: string) => v !== settings.account_creation && choose({ account_creation: v as AccountCreation })}
                options={accountOptions()}
              />
            </div>
          }
        />
        {usesDomains && (
          <SettingsRow
            title={t("instance.allowedDomains", "Allowed domains")}
            description={domainSummary(settings.allowed_domains)}
            control={
              <Button size="sm" onClick={() => setEditing({ pending: null })}>
                {t("instance.editDomains", "Edit domains")}
              </Button>
            }
          />
        )}
        <SettingsRow
          title={t("instance.orgCreation", "Organization creation")}
          description={t("instance.orgCreationHint", "Instance admins can always create organizations.")}
          control={
            <div className="w-240">
              <Select
                aria-label={t("instance.orgCreation", "Organization creation")}
                size="sm"
                fullWidth
                value={settings.org_creation}
                disabled={save.isPending}
                onValueChange={(v: string) => v !== settings.org_creation && choose({ org_creation: v as OrgCreation })}
                options={orgOptions()}
              />
            </div>
          }
        />
      </SettingsGroup>
      {editing && (
        <DomainsDialog
          current={settings.allowed_domains}
          required={usesDomains || editing.pending !== null}
          adding={editing.pending !== null}
          saving={save.isPending}
          onClose={() => setEditing(null)}
          onSave={(allowed_domains) => save.mutate({ ...editing.pending, allowed_domains })}
        />
      )}
    </SettingsSection>
  );
}

function DomainsDialog({
  current,
  required,
  adding,
  saving,
  onClose,
  onSave,
}: {
  current: string[];
  required: boolean;
  adding: boolean;
  saving: boolean;
  onClose: () => void;
  onSave: (domains: string[]) => void;
}) {
  const [domains, setDomains] = useState(current);
  const reason = domains.some((d) => !looksLikeDomain(d))
    ? t("instance.removeNonDomains", "Remove the entries that aren't domains")
    : required && domains.length === 0
      ? t("instance.addDomain", "Add at least one domain")
      : !adding && domains.join() === current.join()
        ? t("instance.noChange", "No change to save")
        : undefined;
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      size="sm"
      title={adding ? t("instance.allowDomainsTitle", "Allow email domains") : t("instance.allowedDomains", "Allowed domains")}
      description={t("instance.domainsHint", "People with an address on these domains qualify; *.example.com covers its subdomains.")}
      onSubmit={() => !reason && onSave(domains)}
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel", "Cancel")}</Button>
          <Button variant="primary" disabled={!!reason} {...(reason ? { disabledReason: reason } : {})} loading={saving} onClick={() => onSave(domains)}>
            {adding ? t("instance.allowDomains", "Allow domains") : t("instance.saveDomains", "Save domains")}
          </Button>
        </>
      }
    >
      <TagInput
        aria-label={t("instance.allowedDomains", "Allowed domains")}
        placeholder={EXAMPLE_DOMAIN}
        value={domains}
        onValueChange={(tags) => setDomains([...new Set(tags.map((tag) => tag.trim()).filter(Boolean))])}
        separators={[",", " ", ";"]}
        validate={(tag) => (looksLikeDomain(tag) ? true : t("instance.notADomain", "Not a domain, like example.com or *.example.com"))}
      />
    </Dialog>
  );
}
