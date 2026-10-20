import { t } from "~/i18n";
import { toast } from "~/shell/toast";

const MESSAGES: Record<string, () => string> = {
  sso_off: () => t("signIn.ssoError.off", "Single sign-on is turned off on this install."),
  no_sso: () => t("signIn.ssoError.noSso", "No organization uses single sign-on for this sign-in; sign in another way."),
  name_org: () => t("signIn.ssoError.nameOrg", "Several organizations use single sign-on here; open your organization's sign-in link."),
  invalid_link: () => t("signIn.ssoError.invalidLink", "This sign-in link isn't valid; start again from the sign-in page."),
  expired: () => t("signIn.ssoError.expired", "The sign-in expired or began in another browser; start again."),
  incomplete: () => t("signIn.ssoError.incomplete", "The identity provider's answer was incomplete; start again."),
  provider_refused: () => t("signIn.ssoError.providerRefused", "The identity provider refused the sign-in."),
  provider_unreachable: () => t("signIn.ssoError.providerUnreachable", "The identity provider couldn't be reached; try again shortly."),
  answer_unverified: () => t("signIn.ssoError.answerUnverified", "The identity provider's answer couldn't be verified; start again."),
  provider_incompatible: () =>
    t("signIn.ssoError.providerIncompatible", "The single sign-on connection doesn't work with this identity provider; tell an admin."),
  email_unverified: () => t("signIn.ssoError.emailUnverified", "The identity provider didn't confirm that your email address is verified."),
  invalid_email: () => t("signIn.ssoError.invalidEmail", "The identity provider sent an email address that isn't valid."),
  domain_not_allowed: () => t("signIn.ssoError.domain", "This organization's single sign-on doesn't sign in addresses at your domain."),
  no_verified_domain: () => t("signIn.ssoError.noVerifiedDomain", "This organization hasn't verified an email domain for single sign-on yet; ask an admin."),
  deactivated: () => t("signIn.ssoError.deactivated", "This account is deactivated."),
  no_access: () => t("signIn.ssoError.noAccess", "You have no access to this organization yet; ask an admin to invite you."),
  accept_invite: () => t("signIn.ssoError.acceptInvite", "You already have an account; sign in another way to accept the invitation."),
  link_required: () => t("signIn.ssoError.linkRequired", "Your account isn't linked to this single sign-on; sign in another way and link it under Account."),
  linked_elsewhere: () => t("signIn.ssoError.linkedElsewhere", "This identity is already linked to another Kaeso account."),
  org_scheduled: () => t("signIn.ssoError.orgScheduled", "This organization is scheduled for deletion."),
  throttled: () => t("signIn.ssoError.throttled", "Too many sign-in attempts; try again in a few minutes."),
};

export function ssoErrorMessage(code: string): string {
  return Object.hasOwn(MESSAGES, code) ? (MESSAGES[code] as () => string)() : t("signIn.ssoError.failed", "Single sign-on didn't work; try again.");
}

export function showSsoError(code: string, id = "sso-error"): void {
  toast.error(ssoErrorMessage(code), { id });
}
