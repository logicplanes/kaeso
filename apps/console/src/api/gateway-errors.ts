import { formatDate, formatNumber } from "@logicplanes/kaeso-ui/format";
import { locale, t } from "~/i18n";
import english from "~/i18n/locales/en/errors.json";

type Values = Record<string, string>;
type Convert = "limit" | "who" | "whoList" | "number" | "date" | "codeList" | "toolChoice" | "timeoutStage";

type ErrorKey = keyof typeof english;

interface Rule {
  key: ErrorKey;
  text: (values: Values) => string;
  convert?: Readonly<Record<string, Convert>>;
}

interface Compiled extends Rule {
  pattern: RegExp;
}

const rule = (key: ErrorKey, text: (values: Values) => string, convert?: Readonly<Record<string, Convert>>): Rule =>
  convert ? { key, text, convert } : { key, text };

const messages = (): Rule[] => [
  rule("errors.keyDisabledByPerson", () =>
    t("errors.keyDisabledByPerson", "A person disabled this key, so only a person can enable it; use a console session."),
  ),
  rule("errors.keyDisabledBySomeoneElse", () =>
    t("errors.keyDisabledBySomeoneElse", "Someone else disabled this key; ask a holder of `keys.all.manage` to enable it."),
  ),
  rule("errors.keysCantMintPlatformKeys", () => t("errors.keysCantMintPlatformKeys", "Keys can't create keys with platform access; use a console session.")),
  rule("errors.signInFirst", () => t("errors.signInFirst", "Sign in first.")),
  rule("errors.sessionEnded", () => t("errors.sessionEnded", "Your session has ended; sign in again.")),
  rule("errors.recentSignIn", () => t("errors.recentSignIn", "This action needs a sign-in from the last 15 minutes; sign in again.")),
  rule("errors.consoleSessionOnly", () => t("errors.consoleSessionOnly", "This needs a console session; API keys cannot manage accounts or the instance.")),
  rule("errors.instanceAdminsOnly", () => t("errors.instanceAdminsOnly", "Only instance admins can do this.")),
  rule("errors.missingRightOrg", (v) => t("errors.missingRightOrg", "You need the right `{right}` in this organization; ask one of its admins for it.", v)),
  rule("errors.missingRightProject", (v) => t("errors.missingRightProject", "You need the right `{right}` in this project; ask one of its admins for it.", v)),
  rule("errors.bearerKey", () => t("errors.bearerKey", "Send the key as `Authorization: Bearer <key>`.")),
  rule("errors.keyNoPlatformAccess", () => t("errors.keyNoPlatformAccess", "This key has no Platform API access; use a key with platform access.")),
  rule("errors.crossSiteOrigin", (v) => t("errors.crossSiteOrigin", "Open Kaeso at {expected}; it refuses requests from pages at other addresses.", v)),
  rule("errors.projectKeyOwnProject", () => t("errors.projectKeyOwnProject", "Project API keys work only in their own project; use an org platform key.")),
  rule("errors.orgRequiresSso", () => t("errors.orgRequiresSso", "This organization requires signing in with SSO.")),
  rule("errors.otherSignIn", () => t("errors.otherSignIn", "You signed in with SSO; sign in with your password or a sign-in link to do this.")),
  rule("errors.ssoSessionOnly", () => t("errors.ssoSessionOnly", "Only a console session can change SSO; API keys can't.")),
  rule("errors.ssoOnly", () => t("errors.ssoOnly", "Your organization requires single sign-on; continue with its identity provider.")),
  rule("errors.adminSsoNoConnection", () => t("errors.adminSsoNoConnection", "This organization has no SSO connection; connect one first.")),
  rule("errors.orgRequires2fa", () => t("errors.orgRequires2fa", "This organization requires two-factor authentication; set it up, then sign in again.")),
  rule("errors.orgReadOnly", () => t("errors.orgReadOnly", "This organization is scheduled for deletion; nothing in it can be changed.")),
  rule("errors.internal", (v) => t("errors.internal", "Something went wrong in the gateway; report request {request} to your admin.", v)),
  rule("errors.databaseUnavailable", () => t("errors.databaseUnavailable", "Kaeso cannot reach its database right now; try again shortly.")),
  rule("errors.somethingWrong", () => t("errors.somethingWrong", "Something went wrong; try again.")),
  rule("errors.noEndpoint", () => t("errors.noEndpoint", "No such endpoint in the Platform API.")),
  rule("errors.methodNotAllowed", (v) => t("errors.methodNotAllowed", "This endpoint does not accept `{method}` requests.", v)),
  rule("errors.notFound.organization", () => t("errors.notFound.organization", "No such organization, or you cannot see it.")),
  rule("errors.notFound.project", () => t("errors.notFound.project", "No such project, or you cannot see it.")),
  rule("errors.notFound.member", () => t("errors.notFound.member", "No such member, or you cannot see it.")),
  rule("errors.notFound.projectMember", () => t("errors.notFound.projectMember", "No such project member, or you cannot see it.")),
  rule("errors.notFound.membership", () => t("errors.notFound.membership", "No such membership, or you cannot see it.")),
  rule("errors.notFound.user", () => t("errors.notFound.user", "No such user, or you cannot see it.")),
  rule("errors.notFound.invite", () => t("errors.notFound.invite", "No such invite, or you cannot see it.")),
  rule("errors.notFound.pendingInvite", () => t("errors.notFound.pendingInvite", "No such pending invite, or you cannot see it.")),
  rule("errors.notFound.role", () => t("errors.notFound.role", "No such role, or you cannot see it.")),
  rule("errors.notFound.orgRight", () => t("errors.notFound.orgRight", "No such org right, or you cannot see it.")),
  rule("errors.notFound.projectRight", () => t("errors.notFound.projectRight", "No such project right, or you cannot see it.")),
  rule("errors.notFound.key", () => t("errors.notFound.key", "No such key, or you cannot see it.")),
  rule("errors.notFound.platformKey", () => t("errors.notFound.platformKey", "No such platform key, or you cannot see it.")),
  rule("errors.notFound.credential", () => t("errors.notFound.credential", "No such credential, or you cannot see it.")),
  rule("errors.notFound.serviceAccount", () => t("errors.notFound.serviceAccount", "No such service account, or you cannot see it.")),
  rule("errors.notFound.activeServiceAccount", () => t("errors.notFound.activeServiceAccount", "No such active service account, or you cannot see it.")),
  rule("errors.notFound.alias", () => t("errors.notFound.alias", "No such alias, or you cannot see it.")),
  rule("errors.notFound.catalogueModel", () => t("errors.notFound.catalogueModel", "No such catalogue model, or you cannot see it.")),
  rule("errors.notFound.limit", () => t("errors.notFound.limit", "No such limit, or you cannot see it.")),
  rule("errors.stale.catalogueModel", () => t("errors.stale.catalogueModel", "This catalogue model was changed since you loaded it; reload it and try again.")),
  rule("errors.stale.limit", () => t("errors.stale.limit", "This limit was changed since you loaded it; reload it and try again.")),
  rule("errors.notFound.logEntry", () => t("errors.notFound.logEntry", "No such log entry, or you cannot see it.")),
  rule("errors.notFound.auditEvent", () => t("errors.notFound.auditEvent", "No such audit event, or you cannot see it.")),
  rule("errors.notFound.picture", () => t("errors.notFound.picture", "No such picture, or you cannot see it.")),
  rule("errors.notFound.session", () => t("errors.notFound.session", "No such session, or you cannot see it.")),
  rule("errors.notFound.ssoConnection", () => t("errors.notFound.ssoConnection", "No such SSO connection, or you cannot see it.")),
  rule("errors.notFound.link", () => t("errors.notFound.link", "No such link, or you cannot see it.")),
  rule("errors.notFound.totpSetup", () => t("errors.notFound.totpSetup", "No such TOTP setup, or you cannot see it.")),
  rule("errors.wrongCredentials", () => t("errors.wrongCredentials", "The email or password is wrong.")),
  rule("errors.tooManySignIns", (v) => t("errors.tooManySignIns", "Too many failed sign-ins; try again in {minutes} minutes or use a sign-in link.", v)),
  rule("errors.signInUnavailable", () => t("errors.signInUnavailable", "Signing in is unavailable right now; try again shortly.")),
  rule("errors.tooManyAttempts", (v) => t("errors.tooManyAttempts", "Too many attempts from your address; try again in {minutes} minutes.", v)),
  rule("errors.passwordSignInOff", () => t("errors.passwordSignInOff", "Password sign-in is turned off on this install.")),
  rule("errors.magicLinkOff", () => t("errors.magicLinkOff", "Sign-in links are turned off on this install.")),
  rule("errors.accountDeactivated", () => t("errors.accountDeactivated", "This account is deactivated.")),
  rule("errors.accountDeactivatedContact", () => t("errors.accountDeactivatedContact", "This account is deactivated; contact your Kaeso administrator.")),
  rule("errors.signInExpired", () => t("errors.signInExpired", "The sign-in has expired; enter your email and password again.")),
  rule("errors.codeOrRecoveryCode", () => t("errors.codeOrRecoveryCode", "Send either `code` or `recovery_code`.")),
  rule("errors.codeInvalidOrUsed", () => t("errors.codeInvalidOrUsed", "That code isn't valid or was already used.")),
  rule("errors.challengeCodesUsedUp", () => t("errors.challengeCodesUsedUp", "Too many wrong codes for this sign-in; start signing in again.")),
  rule("errors.codesLocked", () => t("errors.codesLocked", "Too many wrong codes in a row; sign in with a recovery code or reset your password.")),
  rule("errors.appCodesLocked", () => t("errors.appCodesLocked", "Codes from your app are locked after too many wrong ones; use a recovery code.")),
  rule("errors.enterSecondFactor", () => t("errors.enterSecondFactor", "Enter a code from your authenticator app or a recovery code.")),
  rule("errors.emailCancelLinkInvalid", () => t("errors.emailCancelLinkInvalid", "This link is invalid, used or expired; the change may already be done.")),
  rule("errors.signInLinkInvalid", () => t("errors.signInLinkInvalid", "This sign-in link is invalid, used or expired; request a new one.")),
  rule("errors.resetLinkInvalid", () => t("errors.resetLinkInvalid", "This reset link is invalid, used or expired; request a new one.")),
  rule("errors.signupLinkInvalid", () => t("errors.signupLinkInvalid", "This signup link is invalid, used or expired; request a new one.")),
  rule("errors.confirmLinkInvalid", () => t("errors.confirmLinkInvalid", "This confirmation link is invalid, used or expired.")),
  rule("errors.accountExistsSignIn", () => t("errors.accountExistsSignIn", "An account with this address exists; sign in instead.")),
  rule("errors.alreadySetUp", () => t("errors.alreadySetUp", "Kaeso is already set up; sign in instead.")),
  rule("errors.setupLinkInvalid", () => t("errors.setupLinkInvalid", "The setup link is invalid or expired; run `kaeso setup-link` for a new one.")),
  rule("errors.inviteInvalidAskNew", () => t("errors.inviteInvalidAskNew", "This invitation is no longer valid; ask for a new one.")),
  rule("errors.inviteInvalid", () => t("errors.inviteInvalid", "This invitation is no longer valid.")),
  rule("errors.inviteForOther", (v) => t("errors.inviteForOther", "This invitation is for {email}; sign in with that address to accept it.", v)),
  rule("errors.inviteAccountExists", (v) => t("errors.inviteAccountExists", "An account with {email} exists; sign in to accept the invitation.", v)),
  rule("errors.ssoNoOrg", () => t("errors.ssoNoOrg", "No organization uses SSO for this sign-in; use another way to sign in.")),
  rule("errors.ssoOrgOrEmail", () => t("errors.ssoOrgOrEmail", "Send either `org` or `email`.")),
  rule("errors.ssoEnterEmail", () => t("errors.ssoEnterEmail", "Enter your email to sign in with single sign-on.")),
  rule("errors.nextPath", () => t("errors.nextPath", "`next` must be a console path on this install, starting with one `/`.")),
  rule("errors.setupPasswordOff", () => t("errors.setupPasswordOff", "Password sign-in is turned off on this install; leave `password` out.")),
  rule("errors.idpRefused", () => t("errors.idpRefused", "The identity provider refused the sign-in.")),
  rule("errors.idpIncomplete", () => t("errors.idpIncomplete", "The identity provider's answer is incomplete.")),
  rule("errors.idpInvalidEmail", () => t("errors.idpInvalidEmail", "The identity provider sent an invalid email address.")),
  rule("errors.idpUnreachable", () => t("errors.idpUnreachable", "The identity provider could not be reached.")),
  rule("errors.idpUnverified", () => t("errors.idpUnverified", "The identity provider's answer could not be verified.")),
  rule("errors.idpIncompatible", () => t("errors.idpIncompatible", "The SSO connection doesn't work with this identity provider.")),
  rule("errors.ssoExpired", () => t("errors.ssoExpired", "The SSO sign-in expired or began in another browser; start again.")),
  rule("errors.ssoDomains", (v) => t("errors.ssoDomains", "This organization's SSO only signs in addresses of {domains}.", v)),
  rule("errors.idpEmailUnverified", () => t("errors.idpEmailUnverified", "The identity provider didn't confirm that the email address is verified.")),
  rule("errors.ssoSecretAgain", () => t("errors.ssoSecretAgain", "Send `client_secret` again when you change `issuer` or `client_id`.")),
  rule("errors.issuerPlain", () => t("errors.issuerPlain", "`issuer` takes no query, fragment or credentials.")),
  rule("errors.ssoLinkRequired", () => t("errors.ssoLinkRequired", "Your account isn't linked to this SSO; sign in another way and link it under Account.")),
  rule("errors.ssoLinkedElsewhere", () => t("errors.ssoLinkedElsewhere", "This SSO identity is already linked to another Kaeso account.")),
  rule("errors.ssoSignedInAsOther", () => t("errors.ssoSignedInAsOther", "This browser is signed in to another account; sign out first, then sign in again.")),
  rule("errors.ssoOtherAccount", () => t("errors.ssoOtherAccount", "You signed in at the identity provider as someone else; sign in there as yourself.")),
  rule("errors.ssoStaleSignIn", () => t("errors.ssoStaleSignIn", "The identity provider didn't ask you to sign in again; try again or confirm another way.")),
  rule("errors.ssoNoAuthTime", () => t("errors.ssoNoAuthTime", "The identity provider can't confirm a recent sign-in; use your password or an email link.")),
  rule("errors.noOrgAccess", () => t("errors.noOrgAccess", "You have no access to this organization yet; ask an admin to invite you.")),
  rule("errors.sent.signInLink", () => t("errors.sent.signInLink", "If an account exists for this address, we sent it a sign-in link.")),
  rule("errors.sent.resetLink", () => t("errors.sent.resetLink", "If an account exists for this address, we sent it a password reset link.")),
  rule("errors.sent.signupLink", (v) => t("errors.sent.signupLink", "Open the link we sent to {email} to continue.", v)),
  rule("errors.sent.confirmEmail", (v) => t("errors.sent.confirmEmail", "Open the link we sent to {email} to confirm the change.", v)),
  rule("errors.passwordTooShort", (v) => t("errors.passwordTooShort", "Passwords need at least {min} characters.", v)),
  rule("errors.passwordTooLong", (v) => t("errors.passwordTooLong", "Passwords can have at most {max} characters.", v)),
  rule("errors.currentPasswordWrong", () => t("errors.currentPasswordWrong", "The current password is wrong.")),
  rule("errors.enterCurrentPassword", () => t("errors.enterCurrentPassword", "Enter your current password.")),
  rule("errors.twoFactorAlreadyOn", () => t("errors.twoFactorAlreadyOn", "Two-factor authentication is already on; turn it off to set it up again.")),
  rule("errors.codeInvalidClock", () => t("errors.codeInvalidClock", "That code isn't valid; check your device's clock and try the next one.")),
  rule("errors.orgRequires2faCannotOff", (v) =>
    t("errors.orgRequires2faCannotOff", "The organization {org} requires two-factor authentication, so it cannot be turned off.", v),
  ),
  rule("errors.turnOn2faFirst", () => t("errors.turnOn2faFirst", "Turn on two-factor authentication first.")),
  rule("errors.addressInUse", () => t("errors.addressInUse", "Another account uses this address.")),
  rule("errors.addressInUseNow", () => t("errors.addressInUseNow", "Another account uses this address now.")),
  rule("errors.tooManyEmails", () => t("errors.tooManyEmails", "Too many emails to this address; try again later.")),
  rule("errors.transferBeforeDeleteAccount", (v) => t("errors.transferBeforeDeleteAccount", "Transfer ownership of {orgs} before you delete the account.", v)),
  rule("errors.lastOwnerLeaving", () => t("errors.lastOwnerLeaving", "You are this organization's last owner; transfer ownership before leaving.")),
  rule("errors.invalidEmail", (v) => t("errors.invalidEmail", "`{value}` is not a valid email address.", v)),
  rule("errors.pictureNotBase64", () => t("errors.pictureNotBase64", "The picture isn't valid base64.")),
  rule("errors.pictureEmpty", () => t("errors.pictureEmpty", "The picture is empty.")),
  rule("errors.pictureTooLarge", () => t("errors.pictureTooLarge", "The picture is larger than 512 KiB.")),
  rule("errors.pictureType", () => t("errors.pictureType", "The picture must be a PNG, JPEG or WebP image.")),
  rule("errors.pictureMismatch", (v) => t("errors.pictureMismatch", "The picture is {actual}, not {declared}.", v)),
  rule("errors.onlyOwnersInviteOwners", () => t("errors.onlyOwnersInviteOwners", "Only owners can invite owners.")),
  rule("errors.onlyOwnersMakeOwners", () => t("errors.onlyOwnersMakeOwners", "Only owners can make someone an owner or change an owner's role.")),
  rule("errors.onlyOwnersRemoveOwners", () => t("errors.onlyOwnersRemoveOwners", "Only owners can remove an owner.")),
  rule("errors.lastOwnerMakeAnother", () => t("errors.lastOwnerMakeAnother", "This is the organization's last owner; make someone else an owner first.")),
  rule("errors.lastOwnerTransfer", () => t("errors.lastOwnerTransfer", "This is the organization's last owner; transfer ownership first.")),
  rule("errors.alreadyOrgMember", (v) => t("errors.alreadyOrgMember", "{email} is already a member of this organization.", v)),
  rule("errors.projectNeedsRole", () => t("errors.projectNeedsRole", "Each project needs a `role`.")),
  rule("errors.noActiveProject", () => t("errors.noActiveProject", "No such active project in this organization.")),
  rule("errors.chooseAnotherMember", () => t("errors.chooseAnotherMember", "Choose another member.")),
  rule("errors.newOwnerActiveMember", () => t("errors.newOwnerActiveMember", "The new owner must be an active member of the organization.")),
  rule("errors.memberHoldsMoreRights", () => t("errors.memberHoldsMoreRights", "You can't change a member who holds rights you don't hold.")),
  rule("errors.pendingJoinsHoldMore", () =>
    t("errors.pendingJoinsHoldMore", "Pending invitations or SSO sign-ups would get rights you don't hold in this project."),
  ),
  rule("errors.serviceAccountHoldsMoreRights", () =>
    t("errors.serviceAccountHoldsMoreRights", "You can't change a service account that holds rights you don't hold."),
  ),
  rule("errors.orgAlreadyScheduled", () => t("errors.orgAlreadyScheduled", "The organization is already scheduled for deletion.")),
  rule("errors.orgNotScheduled", () => t("errors.orgNotScheduled", "The organization is not scheduled for deletion.")),
  rule("errors.onlyScheduledDeleteNow", () => t("errors.onlyScheduledDeleteNow", "Only an organization scheduled for deletion can be deleted now.")),
  rule("errors.noOrgDeletion", () => t("errors.noOrgDeletion", "This instance doesn't allow deleting organizations.")),
  rule("errors.restoreBeforeOwner", () => t("errors.restoreBeforeOwner", "Restore the organization before appointing an owner.")),
  rule("errors.ownerActiveUser", () => t("errors.ownerActiveUser", "The owner must be an active user.")),
  rule("errors.newOwnerActiveUser", () => t("errors.newOwnerActiveUser", "The new owner must be an active user.")),
  rule("errors.rolePresetOldName", () => t("errors.rolePresetOldName", "Send `role` only; `preset` is its old name.")),
  rule("errors.roleNotRoleId", () => t("errors.roleNotRoleId", "`role` is not a role id.")),
  rule("errors.noSuchRole", () => t("errors.noSuchRole", "No such role in this organization; see its roles.")),
  rule("errors.projectRoleGiven", () => t("errors.projectRoleGiven", "This is a project role; choose an org role.")),
  rule("errors.orgRoleGiven", () => t("errors.orgRoleGiven", "This is an org role; choose a project role.")),
  rule("errors.scopeOrgOrProject", () => t("errors.scopeOrgOrProject", "`scope` must be org or project.")),
  rule("errors.projectRightsOrgOnly", () => t("errors.projectRightsOrgOnly", "Only org roles have `project_rights`.")),
  rule("errors.roleHoldsMoreRights", () => t("errors.roleHoldsMoreRights", "You can't change a role with rights you don't hold.")),
  rule("errors.roleNameLength", () => t("errors.roleNameLength", "`name` must be 1 to 50 characters.")),
  rule("errors.builtinRoleName", () => t("errors.builtinRoleName", "That's the name of a built-in role; choose another.")),
  rule("errors.roleDescriptionLength", () => t("errors.roleDescriptionLength", "`description` must be at most 200 characters.")),
  rule("errors.roleNameTaken", () => t("errors.roleNameTaken", "A custom role with this name already exists.")),
  rule("errors.maxCustomRoles", (v) => t("errors.maxCustomRoles", "An organization can have at most {max} custom roles.", v)),
  rule("errors.builtinRolesFixed", () => t("errors.builtinRolesFixed", "Built-in roles can't be changed; create a custom role instead.")),
  rule("errors.roleHasHolders", (v) => t("errors.roleHasHolders", "{holders} members, service accounts or invites hold this role; reassign them first.", v)),
  rule("errors.roleIsSsoDefault", () => t("errors.roleIsSsoDefault", "This role is the default role of SSO sign-ins; choose another one first.")),
  rule("errors.grantExceedsOne", (v) => t("errors.grantExceedsOne", "You can't grant `{right}`: you don't hold it yourself.", v)),
  rule("errors.grantExceedsTwo", (v) => t("errors.grantExceedsTwo", "You can't grant `{right}` and 1 more right: you don't hold them yourself.", v)),
  rule("errors.grantExceedsMany", (v) => t("errors.grantExceedsMany", "You can't grant `{right}` and {more} more rights: you don't hold them yourself.", v)),
  rule("errors.rightNeeds", (v) => t("errors.rightNeeds", "`{right}` needs `{needs}`.", v)),
  rule("errors.rightNeedsRemoved", (v) => t("errors.rightNeedsRemoved", "`{right}` needs `{needs}`; remove `{right}` too.", v)),
  rule("errors.ownerOnlyRightAdded", (v) => t("errors.ownerOnlyRightAdded", "`{right}` is held only by owners; make the member an owner instead.", v)),
  rule("errors.ownerOnlyRightInRole", (v) => t("errors.ownerOnlyRightInRole", "`{right}` is held only by owners, so a role can't include it.", v)),
  rule("errors.roleHoldersNeed", (v) => t("errors.roleHoldersNeed", "`{right}` needs `{needs}` for {count} of this role's holders; keep it in the role.", v), {
    count: "number",
  }),
  rule("errors.roleHoldersLack", (v) => t("errors.roleHoldersLack", "`{right}` needs `{needs}`, removed from {count} of this role's holders; restore it.", v), {
    count: "number",
  }),
  rule("errors.notARight", (v) => t("errors.notARight", "`{value}` is not a right; see the rights catalogue.", v)),
  rule("errors.notAPreset", (v) => t("errors.notAPreset", "`{value}` is not a preset.", v)),
  rule("errors.issuerUrl", () => t("errors.issuerUrl", "`issuer` must be a URL.")),
  rule("errors.issuerHttps", () => t("errors.issuerHttps", "`issuer` must be an https URL.")),
  rule("errors.clientSecretRequired", () => t("errors.clientSecretRequired", "`client_secret` is required for a new connection.")),
  rule("errors.notADomain", (v) => t("errors.notADomain", "`{domain}` is not a domain.", v)),
  rule("errors.notADomainExample", (v) => t("errors.notADomainExample", "`{domain}` isn't a domain; use example.com or *.example.com.", v)),
  rule("errors.ssoNeedsDomain", () => t("errors.ssoNeedsDomain", "Name at least one email domain whose users sign in through this provider.")),
  rule("errors.ssoNoOwners", () => t("errors.ssoNoOwners", "SSO can't make owners; choose another role.")),
  rule("errors.domainTaken", (v) => t("errors.domainTaken", "The domain {domain} is verified by another organization.", v)),
  rule("errors.ssoPublicMail", (v) => t("errors.ssoPublicMail", "{domain} is a public mail domain; name your organization's own domains.", v)),
  rule("errors.ssoNoVerifiedDomain", () =>
    t("errors.ssoNoVerifiedDomain", "This organization's SSO has no verified email domain yet; ask an admin to verify one."),
  ),
  rule("errors.dnsUnreadable", () => t("errors.dnsUnreadable", "The domain's DNS records couldn't be read; try again shortly.")),
  rule("errors.ssoRequiredStopFirst", () => t("errors.ssoRequiredStopFirst", "SSO is required in this organization; stop requiring it first.")),
  rule("errors.setUpSsoFirst", () => t("errors.setUpSsoFirst", "Set up an SSO connection before requiring SSO.")),
  rule("errors.keyLifetimeRange", () => t("errors.keyLifetimeRange", "Use 1 to 3650 days, or null for no limit.")),
  rule("errors.alreadyHasAccount", (v) => t("errors.alreadyHasAccount", "{email} already has an account.", v)),
  rule("errors.cannotDeactivateSelf", () => t("errors.cannotDeactivateSelf", "You cannot deactivate your own account here.")),
  rule("errors.cannotDeleteSelf", () => t("errors.cannotDeleteSelf", "You cannot delete your own account here.")),
  rule("errors.onlyActiveDeactivated", () => t("errors.onlyActiveDeactivated", "Only active users can be deactivated.")),
  rule("errors.onlyDeactivatedReactivated", () => t("errors.onlyDeactivatedReactivated", "Only deactivated users can be reactivated.")),
  rule("errors.onlyActiveInstanceAdmins", () => t("errors.onlyActiveInstanceAdmins", "Only active users can be instance admins.")),
  rule("errors.lastInstanceAdmin", () => t("errors.lastInstanceAdmin", "This is the last instance admin; make someone else an instance admin first.")),
  rule("errors.modelsDevUnavailable", () => t("errors.modelsDevUnavailable", "The models.dev data could not be fetched; the gateway log says why.")),
  rule("errors.allowedDomainNeeded", () => t("errors.allowedDomainNeeded", "Add at least one allowed domain to use it.")),
  rule("errors.retentionRange", () => t("errors.retentionRange", "Retention must be 1 to 36,500 days.")),
  rule("errors.gracePeriodRange", () => t("errors.gracePeriodRange", "The grace period must be 1 to 90 days.")),
  rule("errors.contentNotLonger", () => t("errors.contentNotLonger", "Content cannot be kept longer than metadata.")),
  rule("errors.contentNeverLonger", () => t("errors.contentNeverLonger", "Content is never kept longer than metadata.")),
  rule("errors.defaultAboveMaximum", () => t("errors.defaultAboveMaximum", "A default retention cannot exceed its maximum.")),
  rule("errors.maxDomains", (v) => t("errors.maxDomains", "Allow at most {max} domains.", v)),
  rule("errors.nameRecipient", () => t("errors.nameRecipient", "Name a recipient.")),
  rule("errors.emailNotSent", () => t("errors.emailNotSent", "The email was not sent; check the mail settings and the gateway log.")),
  rule("errors.usageUnavailable", () => t("errors.usageUnavailable", "Usage is unavailable right now; try again shortly.")),
  rule("errors.jsonContentType", () => t("errors.jsonContentType", "Send the body as JSON with `Content-Type: application/json`.")),
  rule("errors.bodyNotJson", () => t("errors.bodyNotJson", "The body is not valid JSON.")),
  rule("errors.bodyTooLarge", () => t("errors.bodyTooLarge", "The request body is larger than this install allows.")),
  rule("errors.bodyUnreadable", () => t("errors.bodyUnreadable", "The body could not be read.")),
  rule("errors.bodyShape", () => t("errors.bodyShape", "The body does not have the shape this endpoint expects.")),
  rule("errors.queryForm", () => t("errors.queryForm", "The query string does not have the form this endpoint expects.")),
  rule("errors.pathForm", () => t("errors.pathForm", "The path does not have the form this endpoint expects.")),
  rule("errors.pathParamForm", (v) => t("errors.pathParamForm", "`{name}` in the path does not have the expected form.", v)),
  rule("errors.pathParamUtf8", (v) => t("errors.pathParamUtf8", "`{name}` in the path is not valid UTF-8.", v)),
  rule("errors.fieldRequired", (v) => t("errors.fieldRequired", "`{name}` is required.", v)),
  rule("errors.fieldUnknown", (v) => t("errors.fieldUnknown", "`{name}` is not a field of this request.", v)),
  rule("errors.fieldGivenTwice", (v) => t("errors.fieldGivenTwice", "`{name}` is given more than once.", v)),
  rule("errors.fieldWrongType", (v) => t("errors.fieldWrongType", "`{name}` has a value of the wrong type or form.", v)),
  rule("errors.fieldTrueOrFalse", (v) => t("errors.fieldTrueOrFalse", "`{name}` must be `true` or `false`.", v)),
  rule("errors.fieldWholeNumber", (v) => t("errors.fieldWholeNumber", "`{name}` must be a whole number.", v)),
  rule("errors.fieldOutOfRange", (v) => t("errors.fieldOutOfRange", "`{name}` is out of range.", v)),
  rule("errors.fieldInvalidId", (v) => t("errors.fieldInvalidId", "`{name}` is not a valid id.", v)),
  rule("errors.fieldIdPrefix", (v) => t("errors.fieldIdPrefix", "`{name}` must be an id starting with `{prefix}`.", v)),
  rule("errors.fieldTime", (v) => t("errors.fieldTime", "`{name}` must be a time such as 2026-10-01T00:00:00Z.", v)),
  rule("errors.fieldDate", (v) => t("errors.fieldDate", "`{name}` must be a date, YYYY-MM-DD.", v)),
  rule("errors.fieldLength100", (v) => t("errors.fieldLength100", "`{name}` must be 1 to 100 characters.", v)),
  rule("errors.fieldPositive", (v) => t("errors.fieldPositive", "`{name}` must be positive.", v)),
  rule("errors.limitParamRange", (v) => t("errors.limitParamRange", "`limit` must be between 1 and {max}.", v)),
  rule("errors.afterNotCursor", () => t("errors.afterNotCursor", "`after` is not a cursor from a previous page.")),
  rule("errors.fromBeforeTo", () => t("errors.fromBeforeTo", "`from` must be before `to`.")),
  rule("errors.toAfterFrom", () => t("errors.toAfterFrom", "`to` must be after `from`.")),
  rule("errors.fromOnOrBeforeTo", () => t("errors.fromOnOrBeforeTo", "`from` must be on or before `to`.")),
  rule("errors.logsUnavailable", () => t("errors.logsUnavailable", "Request logs are unavailable right now; try again shortly.")),
  rule("errors.daysRangeInstall", (v) => t("errors.daysRangeInstall", "Must be 1 to {max} days on this install.", v)),
  rule("errors.orgContentLoggingOff", () => t("errors.orgContentLoggingOff", "The org has content logging off; a project cannot switch it on.")),
  rule("errors.noProjectInOrg", () => t("errors.noProjectInOrg", "No such project in this org.")),
  rule("errors.sourceValues", () => t("errors.sourceValues", "`source` must be `api` or `playground`.")),
  rule("errors.exportFormat", () => t("errors.exportFormat", "Use `jsonl` or `csv`.")),
  rule("errors.csvMetadataOnly", () => t("errors.csvMetadataOnly", "CSV exports carry metadata only.")),
  rule("errors.exportContentRight", () => t("errors.exportContentRight", "Exporting content needs the right `logs.content.view`.")),
  rule("errors.usageGroupValues", () => t("errors.usageGroupValues", "Use `day`, `week`, `month`, `model`, `key`, `member`, or `project` for orgs.")),
  rule("errors.usageByDayRange", () => t("errors.usageByDayRange", "By day, `from` and `to` are at most 400 days apart; use `week` or `month`.")),
  rule("errors.projectArchived", () => t("errors.projectArchived", "The project is archived and cannot be changed.")),
  rule("errors.projectArchivedKeys", () => t("errors.projectArchivedKeys", "The project is archived; its keys cannot change.")),
  rule("errors.projectArchivedModels", () => t("errors.projectArchivedModels", "The project is archived; its models cannot change.")),
  rule("errors.projectArchivedCalls", () => t("errors.projectArchivedCalls", "The project is archived; its models cannot be called.")),
  rule("errors.orgMaxDays", (v) => t("errors.orgMaxDays", "The organization allows at most {max} days.", v)),
  rule("errors.defaultProjectArchive", () => t("errors.defaultProjectArchive", "The default project cannot be archived; make another one the default first.")),
  rule("errors.archivedNotDefault", () => t("errors.archivedNotDefault", "An archived project cannot be the default.")),
  rule("errors.onlyActiveMembersJoin", () => t("errors.onlyActiveMembersJoin", "Only active members of the organization can join its projects.")),
  rule("errors.alreadyProjectMember", () => t("errors.alreadyProjectMember", "The user is already a member of this project.")),
  rule("errors.notProjectMember", () => t("errors.notProjectMember", "This user is not a member of the project.")),
  rule("errors.serviceAccountArchived", () => t("errors.serviceAccountArchived", "The service account is archived.")),
  rule("errors.serviceAccountAlreadyArchived", () => t("errors.serviceAccountAlreadyArchived", "The service account is already archived.")),
  rule("errors.serviceAccountKeysPath", (v) => t("errors.serviceAccountKeysPath", "Service accounts get keys through service-accounts/{account}/keys.", v)),
  rule("errors.keyArchived", () => t("errors.keyArchived", "The key is archived and cannot change.")),
  rule("errors.keyChangedMeanwhile", () => t("errors.keyChangedMeanwhile", "The key changed meanwhile; reload and try again.")),
  rule("errors.platformAccessRight", () =>
    t("errors.platformAccessRight", "Giving a key platform access needs the right `keys.platform_access.grant` in this project."),
  ),
  rule("errors.ownKeysRight", () => t("errors.ownKeysRight", "You need the right `keys.own.manage` in this key's project to change it.")),
  rule("errors.noPlatformKeyToChange", () => t("errors.noPlatformKeyToChange", "No such platform key that can change.")),
  rule("errors.expiryBoth", () => t("errors.expiryBoth", "Send either `expires_in_days` or `expires_on`, not both.")),
  rule("errors.expiryChoice", () => t("errors.expiryChoice", "Choose an expiry of 1, 7, 30 or 90 days, a date, or never.")),
  rule("errors.expiryPast", () => t("errors.expiryPast", "The expiry date must be in the future.")),
  rule("errors.keyMaxDays", (v) => t("errors.keyMaxDays", "Keys in this project may live at most {days} days.", v)),
  rule("errors.keyNeverNotAllowed", (v) => t("errors.keyNeverNotAllowed", 'Keys in this project must expire within {days} days; "never" is not allowed.', v)),
  rule("errors.notActiveProject", (v) => t("errors.notActiveProject", "`{project}` is not an active project of this organization.", v)),
  rule("errors.secretLength", (v) => t("errors.secretLength", "The secret must be {min} to {max} characters.", v)),
  rule("errors.secretCharacters", () => t("errors.secretCharacters", "The secret must not contain spaces or control characters.")),
  rule("errors.providerIdForm", () => t("errors.providerIdForm", "`provider` must be a provider id, such as `openai`.")),
  rule("errors.providerOneOf", (v) => t("errors.providerOneOf", "The provider must be one of {providers}.", v)),
  rule("errors.noModelInCatalogue", () => t("errors.noModelInCatalogue", "No such model in this project's catalogue.")),
  rule("errors.modelNotInCatalogue", (v) => t("errors.modelNotInCatalogue", "The model `{model}` is not in this project's catalogue.", v)),
  rule("errors.notInProjectCatalogue", (v) => t("errors.notInProjectCatalogue", "`{model}` is not in the project's catalogue.", v)),
  rule("errors.notInMemberModels", (v) => t("errors.notInMemberModels", "`{model}` is not in the models the key's member may use.", v)),
  rule("errors.sendAllOrModels", () => t("errors.sendAllOrModels", "Send either `all: true` or a `models` list.")),
  rule("errors.aliasLength", () => t("errors.aliasLength", "An alias must be 1 to 200 characters.")),
  rule("errors.aliasCharacters", () => t("errors.aliasCharacters", "An alias must not contain spaces, control characters or `/`.")),
  rule("errors.aliasTaken", (v) => t("errors.aliasTaken", "The alias `{name}` exists in this project.", v)),
  rule("errors.modelIdForm", () => t("errors.modelIdForm", "A model id has 1 to 200 characters and no spaces.")),
  rule("errors.listModelIds", () => t("errors.listModelIds", "List 1 to 200 model ids.")),
  rule("errors.modelIdsOrAllCurrent", () => t("errors.modelIdsOrAllCurrent", "Send either `model_ids` or `all_current: true`.")),
  rule("errors.notOnModelsDevAddCustom", (v) =>
    t("errors.notOnModelsDevAddCustom", "The model `{model}` is not on models.dev for `{provider}`; add it as a custom model.", v),
  ),
  rule("errors.notOnModelsDevSendCustom", (v) =>
    t("errors.notOnModelsDevSendCustom", "The model `{model}` is not on models.dev for `{provider}`; send `custom: true` to add it anyway.", v),
  ),
  rule("errors.notOnModelsDevAlone", (v) =>
    t("errors.notOnModelsDevAlone", "The model `{model}` is not on models.dev for `{provider}`; add it alone as a custom model.", v),
  ),
  rule("errors.onModelsDev", (v) => t("errors.onModelsDev", "The model `{model}` is on models.dev; add it without `custom`.", v)),
  rule("errors.catalogueHasModel", (v) => t("errors.catalogueHasModel", "The catalogue already has `{provider}/{model}`.", v)),
  rule("errors.notCatalogueField", (v) => t("errors.notCatalogueField", "`{name}` is not a catalogue field.", v)),
  rule("errors.overrideWrongType", () => t("errors.overrideWrongType", "An override has a value of the wrong type.")),
  rule("errors.displayNameLength", () => t("errors.displayNameLength", "The display name must be 1 to 200 characters.")),
  rule("errors.modalityNeeded", (v) => t("errors.modalityNeeded", "`{name}` must list at least one modality.", v)),
  rule("errors.priceRange", (v) => t("errors.priceRange", "`{name}` must be 0 to 1,000,000 USD per million tokens, at most 9 decimals.", v)),
  rule("errors.notDialect", (v) => t("errors.notDialect", "`{dialect}` is not a dialect of {provider}; use one of {dialects}.", v)),
  rule("errors.adaptiveThinking", () => t("errors.adaptiveThinking", "Adaptive thinking applies only to Anthropic models.")),
  rule("errors.timeoutRange", () => t("errors.timeoutRange", "The timeout must be 1 to 3600 seconds.")),
  rule("errors.releaseDate", () => t("errors.releaseDate", "The release date must be a date, YYYY-MM-DD.")),
  rule("errors.customModelNeeds", (v) => t("errors.customModelNeeds", "A custom model needs {fields}.", v)),
  rule("errors.valueNotOneOf", (v) => t("errors.valueNotOneOf", "`{value}` is not one of {allowed}.", v)),
  rule("errors.valueListedTwice", (v) => t("errors.valueListedTwice", "`{value}` is listed twice.", v)),
  rule("errors.projectLimitNoSubject", () => t("errors.projectLimitNoSubject", "A project limit names no `user`, `service_account` or `key`.")),
  rule("errors.memberLimitSubject", () => t("errors.memberLimitSubject", "A member limit names either `user` or `service_account`.")),
  rule("errors.keyLimitSubject", () => t("errors.keyLimitSubject", "A key limit names only `key`.")),
  rule("errors.limitLevelValues", () => t("errors.limitLevelValues", "Use `project`, `member` or `key`.")),
  rule("errors.limitKindValues", () => t("errors.limitKindValues", "Use `request_rate`, `token_rate`, `token_budget` or `spend_budget`.")),
  rule("errors.limitPeriodValues", () => t("errors.limitPeriodValues", "Use `minute`, `day`, `month` or `total`.")),
  rule("errors.amountRequired", () => t("errors.amountRequired", "An amount is required.")),
  rule("errors.ratesPerMinute", () => t("errors.ratesPerMinute", "Rates are per `minute`.")),
  rule("errors.budgetPeriods", () => t("errors.budgetPeriods", "Budgets are per `day`, `month` or `total`.")),
  rule("errors.amountRange", () => t("errors.amountRange", "The amount must be positive and at most 10^12.")),
  rule("errors.spendAmountRange", () => t("errors.spendAmountRange", "The amount must be positive and at most 1,000,000,000 USD.")),
  rule("errors.spendDecimals", () => t("errors.spendDecimals", "A spend budget has at most 9 decimals.")),
  rule("errors.amountWhole", () => t("errors.amountWhole", "The amount must be a whole number.")),
  rule("errors.alertThresholds", () => t("errors.alertThresholds", "Alert thresholds are up to 10 ascending percentages from 1 to 100.")),
  rule("errors.rateLimitsNoAlerts", () => t("errors.rateLimitsNoAlerts", "Rate limits have no alerts; leave `alert_thresholds` out.")),
  rule("errors.noActiveServiceAccountId", () => t("errors.noActiveServiceAccountId", "No active service account with this id in the project.")),
  rule("errors.noActiveKeyId", () => t("errors.noActiveKeyId", "No active key with this id in the project.")),
  rule("errors.limitExists", () => t("errors.limitExists", "This subject already has a limit of this kind, model and period.")),
  rule("errors.limitOncePerKind", () => t("errors.limitOncePerKind", "Each kind, model and period may appear once.")),
  rule("errors.limitAboveParent", (v) => t("errors.limitAboveParent", "This limit is higher than the {limit} of {who} ({amount}).", v), {
    limit: "limit",
    who: "who",
    amount: "number",
  }),
  rule("errors.lowerLimitsFirst", (v) => t("errors.lowerLimitsFirst", "First lower the limits that allow more than this one: {list}.", v), { list: "whoList" }),
  rule("errors.rateLimit.project", (v) => t("errors.rateLimit.project", "Rate limit reached: the project's {limit}; retry in {seconds} seconds.", v), {
    limit: "limit",
  }),
  rule("errors.rateLimit.member", (v) => t("errors.rateLimit.member", "Rate limit reached: the member's {limit}; retry in {seconds} seconds.", v), {
    limit: "limit",
  }),
  rule("errors.rateLimit.key", (v) => t("errors.rateLimit.key", "Rate limit reached: this key's {limit}; retry in {seconds} seconds.", v), { limit: "limit" }),
  rule(
    "errors.nearlyUsedUp.project",
    (v) => t("errors.nearlyUsedUp.project", "The project's {limit} is nearly used up by requests in progress; retry in a moment.", v),
    { limit: "limit" },
  ),
  rule(
    "errors.nearlyUsedUp.member",
    (v) => t("errors.nearlyUsedUp.member", "The member's {limit} is nearly used up by requests in progress; retry in a moment.", v),
    { limit: "limit" },
  ),
  rule("errors.nearlyUsedUp.key", (v) => t("errors.nearlyUsedUp.key", "This key's {limit} is nearly used up by requests in progress; retry in a moment.", v), {
    limit: "limit",
  }),
  rule("errors.budgetResets.project", (v) => t("errors.budgetResets.project", "Budget used up: the project's {limit}; it resets at {time}.", v), {
    limit: "limit",
    time: "date",
  }),
  rule("errors.budgetResets.member", (v) => t("errors.budgetResets.member", "Budget used up: the member's {limit}; it resets at {time}.", v), {
    limit: "limit",
    time: "date",
  }),
  rule("errors.budgetResets.key", (v) => t("errors.budgetResets.key", "Budget used up: this key's {limit}; it resets at {time}.", v), {
    limit: "limit",
    time: "date",
  }),
  rule(
    "errors.budgetNoReset.project",
    (v) => t("errors.budgetNoReset.project", "Budget used up: the project's {limit}; it does not reset, so raise the limit to continue.", v),
    { limit: "limit" },
  ),
  rule(
    "errors.budgetNoReset.member",
    (v) => t("errors.budgetNoReset.member", "Budget used up: the member's {limit}; it does not reset, so raise the limit to continue.", v),
    { limit: "limit" },
  ),
  rule(
    "errors.budgetNoReset.key",
    (v) => t("errors.budgetNoReset.key", "Budget used up: this key's {limit}; it does not reset, so raise the limit to continue.", v),
    { limit: "limit" },
  ),
  rule("errors.unpricedFeature.project", (v) =>
    t("errors.unpricedFeature.project", "The project has a spend budget that can't price `{feature}`; ask an admin.", v),
  ),
  rule("errors.unpricedFeature.member", (v) =>
    t("errors.unpricedFeature.member", "The member has a spend budget that can't price `{feature}`; ask an admin.", v),
  ),
  rule("errors.unpricedFeature.key", (v) => t("errors.unpricedFeature.key", "This key has a spend budget that can't price `{feature}`; ask an admin.", v)),
  rule("errors.hardBudgetsUnavailable", () =>
    t("errors.hardBudgetsUnavailable", "Kaeso cannot check this project's hard budgets right now; try again shortly."),
  ),
  rule("errors.playgroundConsoleOnly", () =>
    t("errors.playgroundConsoleOnly", "The Playground needs a console session; apps call models with a key on `/v1`."),
  ),
  rule("errors.playgroundListConsoleOnly", () =>
    t("errors.playgroundListConsoleOnly", "The Playground needs a console session; apps list models with a key on `/v1`."),
  ),
  rule("errors.conversationTooLarge", () => t("errors.conversationTooLarge", "The conversation is larger than Kaeso accepts.")),
  rule("errors.notResponsesRequest", () => t("errors.notResponsesRequest", "The request is not a Responses request Kaeso can read.")),
  rule("errors.modelsUnavailable", () => t("errors.modelsUnavailable", "Kaeso cannot load this project's models right now; try again shortly.")),
  rule("errors.ambiguousModel", (v) => t("errors.ambiguousModel", "Several providers serve `{model}` here; name it as `provider/{model}`.", v)),
  rule("errors.modelNotAllowed", (v) => t("errors.modelNotAllowed", "The model `{model}` is not among the models you may use in this project.", v)),
  rule("errors.modelUnserved", (v) => t("errors.modelUnserved", "{provider} does not serve `{model}` to this project's key; check the key's access.", v)),
  rule("errors.modelCannotTake", () => t("errors.modelCannotTake", "The model cannot take this request as it is; check its settings.")),
  rule("errors.budgetsUnavailable", () => t("errors.budgetsUnavailable", "Kaeso cannot check this project's budgets right now; try again shortly.")),
  rule("errors.limitRefused", () => t("errors.limitRefused", "A limit of this project refused the request; try again later.")),
  rule("errors.noProviderKey", (v) => t("errors.noProviderKey", "No provider key for {provider} is set up for this project; an org admin can add one.", v)),
  rule("errors.providerKeyUnavailable", () => t("errors.providerKeyUnavailable", "Kaeso cannot load the project's provider key right now; try again shortly.")),
  rule("errors.providerTimeout", () => t("errors.providerTimeout", "The provider did not answer in time; send the request again.")),
  rule("errors.providerUnreachable", () => t("errors.providerUnreachable", "Kaeso cannot reach the provider right now; try again shortly.")),
  rule("errors.providerUnreadable", () => t("errors.providerUnreadable", "The provider's answer could not be read; try again.")),
  rule("errors.restarting", () => t("errors.restarting", "Kaeso is restarting and ended this request; send it again.")),
  rule("errors.contextTooLong", () => t("errors.contextTooLong", "The conversation is longer than the model's context window.")),
  rule("errors.contentFiltered", () => t("errors.contentFiltered", "The provider's content filter refused the request.")),
  rule("errors.providerTooLarge", () => t("errors.providerTooLarge", "The provider refused the request as too large.")),
  rule("errors.providerInvalid", () => t("errors.providerInvalid", "The provider refused the request as invalid; check the model's settings.")),
  rule("errors.providerNotFound", () => t("errors.providerNotFound", "The provider did not find something the request names; check the model and its ids.")),
  rule("errors.providerKeyRefused", () => t("errors.providerKeyRefused", "The provider refused the project's provider key; an org admin can replace it.")),
  rule("errors.providerBilling", () => t("errors.providerBilling", "The provider refused the request for billing reasons; check the provider account.")),
  rule("errors.providerRateLimited", () => t("errors.providerRateLimited", "The provider is rate limiting the provider key; try again shortly.")),
  rule("errors.providerOverloaded", () => t("errors.providerOverloaded", "The provider is overloaded right now; try again shortly.")),
  rule("errors.providerFailed", () => t("errors.providerFailed", "The provider failed to answer; try again shortly.")),
  rule("errors.notFound.update", () => t("errors.notFound.update", "No such update, or you cannot see it.")),
  rule(
    "errors.unknownKeysThrottled",
    (v) => t("errors.unknownKeysThrottled", "Too many requests with unknown API keys from your address; try again in {seconds} seconds.", v),
    {
      seconds: "number",
    },
  ),
  rule("errors.apiKeyInvalid", () => t("errors.apiKeyInvalid", "This API key is not valid; check that it is complete and current.")),
  rule("errors.apiKeyDisabled", () => t("errors.apiKeyDisabled", "This API key is disabled; a project admin can enable it again.")),
  rule("errors.apiKeyArchived", () => t("errors.apiKeyArchived", "This API key was archived and no longer works.")),
  rule("errors.apiKeyExpired", () => t("errors.apiKeyExpired", "This API key has expired.")),
  rule("errors.apiKeyOrgDeleting", () => t("errors.apiKeyOrgDeleting", "This organization is scheduled for deletion; its keys no longer work.")),
  rule("errors.apiKeyProjectArchived", () => t("errors.apiKeyProjectArchived", "This key's project is archived; its keys no longer work.")),
  rule("errors.apiKeyOwnerInactive", () => t("errors.apiKeyOwnerInactive", "The account or service account this key belongs to is no longer active.")),
  rule("errors.limitsUnavailable", () => t("errors.limitsUnavailable", "Kaeso cannot check this project's limits right now; try again shortly.")),
  rule("errors.inviteMaxProjects", (v) => t("errors.inviteMaxProjects", "An invitation can name at most {max} projects.", v), { max: "number" }),
  rule("errors.inviteProjectTwice", () => t("errors.inviteProjectTwice", "Each project can be named only once.")),
  rule("errors.fieldInvisibleCharacters", (v) => t("errors.fieldInvisibleCharacters", "`{name}` can't contain line breaks or invisible characters.", v)),
  rule("errors.logDeletionsWaiting", (v) => t("errors.logDeletionsWaiting", "{count} log deletions are still waiting; try again once they have run.", v), {
    count: "number",
  }),
  rule("errors.ifMatchForm", () => t("errors.ifMatchForm", '`If-Match` must be an entity tag from `ETag`, such as "3".')),
  rule("errors.updates.noUpdater", () => t("errors.updates.noUpdater", "The updater isn't installed; update this install by hand.")),
  rule("errors.updates.notNewest", () => t("errors.updates.notNewest", "That isn't the newest offered release; check for updates again.")),
  rule("errors.updates.manualSteps", () => t("errors.updates.manualSteps", "This release needs steps by hand; follow the upgrade guide.")),
  rule("errors.updates.updaterOutdated", () => t("errors.updates.updaterOutdated", "The updater is too old for this release; update it by hand first.")),
  rule("errors.updates.alreadyRunning", () => t("errors.updates.alreadyRunning", "An update is already running; wait for it to finish.")),
  rule("errors.updates.updaterBusy", () => t("errors.updates.updaterBusy", "The updater is busy with another update; try again later.")),
  rule("errors.updates.updaterRefused", () => t("errors.updates.updaterRefused", "The updater refused the update; nothing was changed.")),
  rule("errors.updates.hourRange", () => t("errors.updates.hourRange", "Choose an hour from 0 to 23.")),
  rule("errors.unknownTimeZone", (v) => t("errors.unknownTimeZone", "`{param}` isn't a known time zone; choose one such as `Europe/Berlin`.", v)),
  rule("errors.updates.updaterUnreachable", () => t("errors.updates.updaterUnreachable", "The updater couldn't be reached; nothing was changed.")),
  rule("errors.model.generationOnly", (v) => t("errors.model.generationOnly", "The model `{model}` is a generation model; it cannot create embeddings.", v)),
  rule("errors.model.endpoint", (v) => t("errors.model.endpoint", "The model `{model}` cannot serve this endpoint.", v)),
  rule("errors.model.noTokenCount", (v) => t("errors.model.noTokenCount", "The model `{model}` cannot count tokens.", v)),
  rule("errors.model.embeddingsOnly", (v) => t("errors.model.embeddingsOnly", "The model `{model}` is an embeddings model; it cannot generate text.", v)),
  rule("errors.model.noText", (v) => t("errors.model.noText", "The model `{model}` does not produce text.", v)),
  rule("errors.model.noImages", (v) => t("errors.model.noImages", "The model `{model}` does not accept images.", v)),
  rule("errors.model.noPdf", (v) => t("errors.model.noPdf", "The model `{model}` does not accept PDF documents.", v)),
  rule("errors.model.noTools", (v) => t("errors.model.noTools", "The model `{model}` does not support tool calling.", v)),
  rule(
    "errors.model.forcedTool",
    (v) => t("errors.model.forcedTool", "The model `{model}` does not accept {choice}; send `auto` and ask for the tool in the prompt.", v),
    { choice: "toolChoice" },
  ),
  rule("errors.model.noStructured", (v) => t("errors.model.noStructured", "The model `{model}` does not support structured output (a JSON schema).", v)),
  rule("errors.model.noReasoning", (v) => t("errors.model.noReasoning", "The model `{model}` does not support reasoning.", v)),
  rule("errors.model.noAdaptive", (v) =>
    t("errors.model.noAdaptive", "The model `{model}` has no adaptive thinking; send `thinking` `enabled` with `budget_tokens` instead.", v),
  ),
  rule("errors.model.noTemperature", (v) => t("errors.model.noTemperature", "The model `{model}` does not accept `temperature` (only the default of 1).", v)),
  rule("errors.model.noEffort", (v) => t("errors.model.noEffort", "The model `{model}` does not accept an effort level; `{effort}` cannot be sent.", v)),
  rule(
    "errors.model.budgetAtLeast",
    (v) => t("errors.model.budgetAtLeast", "A `budget_tokens` of {budget} means `{effort}`; `{model}` needs at least {min}.", v),
    { budget: "number", min: "number" },
  ),
  rule(
    "errors.model.budgetBelow",
    (v) => t("errors.model.budgetBelow", "A `budget_tokens` of {budget} means `{effort}`; `{model}` needs less than {max}.", v),
    { budget: "number", max: "number" },
  ),
  rule("errors.model.budgetOff", (v) => t("errors.model.budgetOff", "A `budget_tokens` of {budget} means `{effort}`; `{model}` needs `thinking` off.", v), {
    budget: "number",
  }),
  rule(
    "errors.model.effortLevels",
    (v) => t("errors.model.effortLevels", "The model `{model}` supports the effort levels {levels}; `{effort}` is not one of them.", v),
    { levels: "codeList" },
  ),
  rule("errors.modelApi.notInCatalogue", (v) =>
    t(
      "errors.modelApi.notInCatalogue",
      "The model `{model}` is not in this project's model catalogue. A project admin can add it under the project's models.",
      v,
    ),
  ),
  rule("errors.modelApi.ambiguous", (v) =>
    t(
      "errors.modelApi.ambiguous",
      "The model name `{model}` is served by several providers in this project ({providers}). Use a prefixed name such as `{provider}/{model}`.",
      v,
    ),
  ),
  rule("errors.modelApi.memberAccess", (v) =>
    t(
      "errors.modelApi.memberAccess",
      "The model `{model}` is not in the models this key's member may use in this project. A project admin can change the member's model access.",
      v,
    ),
  ),
  rule("errors.modelApi.keyAccess", (v) =>
    t(
      "errors.modelApi.keyAccess",
      "The model `{model}` is not in this key's model list. Create a key with access to it, or ask a project admin to change the key's model access.",
      v,
    ),
  ),
  rule("errors.modelApi.sendKey", () => t("errors.modelApi.sendKey", "Send a Kaeso API key as `Authorization: Bearer <key>` or `x-api-key: <key>`.")),
  rule("errors.modelApi.keysUnavailable", () => t("errors.modelApi.keysUnavailable", "Kaeso cannot check API keys right now. Try again shortly.")),
  rule("errors.modelApi.platformKey", () =>
    t("errors.modelApi.platformKey", "Org platform keys manage Kaeso through the Platform API and cannot call models. Use a project API key."),
  ),
  rule("errors.modelApi.platformOnly", () =>
    t("errors.modelApi.platformOnly", "This API key has Platform API access only. Create a key with inference access to call models."),
  ),
  rule("errors.modelApi.noRequestRight", () =>
    t("errors.modelApi.noRequestRight", "The owner of this API key may not call models in its project (the right `api.request` is missing)."),
  ),
  rule("errors.modelApi.modelsUnavailable", () =>
    t("errors.modelApi.modelsUnavailable", "Kaeso cannot load this project's models right now. Try again shortly."),
  ),
  rule("errors.modelApi.noSuchModel", (v) => t("errors.modelApi.noSuchModel", "The model `{model}` does not exist or this key may not use it.", v)),
  rule("errors.modelApi.noRoute", (v) => t("errors.modelApi.noRoute", "Kaeso does not serve `{path}`.", v)),
  rule("errors.modelApi.method", (v) => t("errors.modelApi.method", "`{path}` does not accept `{method}` requests.", v)),
  rule("errors.modelApi.restarting", () => t("errors.modelApi.restarting", "Kaeso is restarting and ended this request. Send it again.")),
  rule("errors.modelApi.appClosed", () => t("errors.modelApi.appClosed", "The app closed the connection before the reply was complete.")),
  rule(
    "errors.modelApi.timeout",
    (v) => t("errors.modelApi.timeout", "The provider did not answer in time ({stage}). Kaeso does not retry; try again or raise the model's timeout.", v),
    { stage: "timeoutStage" },
  ),
  rule("errors.modelApi.unreachable", () => t("errors.modelApi.unreachable", "Kaeso cannot reach the provider. Try again shortly.")),
  rule("errors.modelApi.transfer", () => t("errors.modelApi.transfer", "The connection to the provider failed before the response was complete.")),
  rule("errors.modelApi.prepare", () => t("errors.modelApi.prepare", "Kaeso cannot prepare the call to the provider.")),
  rule("errors.modelApi.responseTooLarge", () => t("errors.modelApi.responseTooLarge", "The provider's response is larger than this install accepts.")),
  rule("errors.modelApi.keyRefused", () =>
    t("errors.modelApi.keyRefused", "The provider refused this project's provider key. Ask an organization admin to check it under Providers."),
  ),
  rule("errors.modelApi.accountRefused", () =>
    t(
      "errors.modelApi.accountRefused",
      "The provider refused this project's provider key for billing or quota reasons. Ask an organization admin to check it under Providers.",
    ),
  ),
  rule("errors.modelApi.translateUnavailable", () =>
    t("errors.modelApi.translateUnavailable", "Kaeso cannot translate this request right now. Try again shortly."),
  ),
  rule("errors.modelApi.unserved", (v) =>
    t(
      "errors.modelApi.unserved",
      "{provider} does not serve the model `{model}` to this project's provider key; check the key's access to it, or remove the model from the catalogue.",
      v,
    ),
  ),
  rule("errors.modelApi.bodyTimeout", () => t("errors.modelApi.bodyTimeout", "The request body did not arrive in time; send it again.")),
  rule("errors.modelApi.bodyTooLarge", (v) => t("errors.modelApi.bodyTooLarge", "The request body is larger than {limit} bytes.", v), { limit: "number" }),
  rule("errors.modelApi.streamOnly", () => t("errors.modelApi.streamOnly", "Only streamed requests are served here; send `stream: true`.")),
  rule("errors.modelApi.providerUnavailable", (v) => t("errors.modelApi.providerUnavailable", "The provider `{provider}` is not available.", v)),
  rule("errors.modelApi.credentialSealed", () =>
    t("errors.modelApi.credentialSealed", "Kaeso cannot decrypt the provider credential. The operator must check the configured master keys."),
  ),
  rule("errors.modelApi.credentialUnavailable", () =>
    t("errors.modelApi.credentialUnavailable", "Kaeso cannot load the provider credential right now. Try again shortly."),
  ),
  rule("errors.modelApi.noBaseUrl", () => t("errors.modelApi.noBaseUrl", "No base URL is configured for this provider.")),
  rule("errors.modelApi.streamEnded", () => t("errors.modelApi.streamEnded", "The provider's stream ended before the reply was complete.")),
  rule("errors.modelApi.streamCapped", () => t("errors.modelApi.streamCapped", "The reply reached this install's limit for a stream; Kaeso ended it.")),
  rule("errors.modelApi.noCredential", (v) =>
    t(
      "errors.modelApi.noCredential",
      "No credential for the provider `{provider}` is set up for this project. An org admin adds a key for it with POST /platform/v1/orgs/{org}/credentials, makes one of the org's keys the default, or assigns one to this project.",
      v,
    ),
  ),
  rule("errors.edgeUnreachable", () => t("errors.edgeUnreachable", "Kaeso can't be reached right now; try again in a minute.")),
  rule("errors.edgeUpdating", () => t("errors.edgeUpdating", "Kaeso is being updated; try again in a minute.")),
];

const toolChoices = (): Rule[] => [rule("errors.toolChoice.named", () => t("errors.toolChoice.named", "a named function in `tool_choice`"))];

const timeoutStages = (): Rule[] => [
  rule("errors.timeoutStage.firstByte", () => t("errors.timeoutStage.firstByte", "time to first byte")),
  rule("errors.timeoutStage.request", () => t("errors.timeoutStage.request", "request")),
  rule("errors.timeoutStage.connect", () => t("errors.timeoutStage.connect", "connect")),
  rule("errors.timeoutStage.idleStream", () => t("errors.timeoutStage.idleStream", "idle stream")),
];

const limits = (): Rule[] => [
  rule("errors.limit.requestRate", (v) => t("errors.limit.requestRate", "request rate of {amount} requests per minute", v), { amount: "number" }),
  rule("errors.limit.tokenRate", (v) => t("errors.limit.tokenRate", "token rate of {amount} tokens per minute", v), { amount: "number" }),
  rule("errors.limit.dailyTokenBudget", (v) => t("errors.limit.dailyTokenBudget", "daily token budget of {amount} tokens", v), { amount: "number" }),
  rule("errors.limit.monthlyTokenBudget", (v) => t("errors.limit.monthlyTokenBudget", "monthly token budget of {amount} tokens", v), { amount: "number" }),
  rule("errors.limit.totalTokenBudget", (v) => t("errors.limit.totalTokenBudget", "total token budget of {amount} tokens", v), { amount: "number" }),
  rule("errors.limit.dailySpendBudget", (v) => t("errors.limit.dailySpendBudget", "daily spend budget of {amount} USD", v), { amount: "number" }),
  rule("errors.limit.monthlySpendBudget", (v) => t("errors.limit.monthlySpendBudget", "monthly spend budget of {amount} USD", v), { amount: "number" }),
  rule("errors.limit.totalSpendBudget", (v) => t("errors.limit.totalSpendBudget", "total spend budget of {amount} USD", v), { amount: "number" }),
  rule("errors.limit.forModel", (v) => t("errors.limit.forModel", "{limit} for this model", v), { limit: "limit" }),
];

const owners = (): Rule[] => [
  rule("errors.limit.who.project", () => t("errors.limit.who.project", "the project")),
  rule("errors.limit.who.member", (v) => t("errors.limit.who.member", "member {id}", v)),
  rule("errors.limit.who.key", (v) => t("errors.limit.who.key", "key {id}", v)),
];

const ownerAmounts = (): Rule[] => [
  rule("errors.limit.whoAmount", (v) => t("errors.limit.whoAmount", "{who} ({amount})", v), { who: "who", amount: "number" }),
];

const escape = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

export function pattern(englishText: string): RegExp {
  const names = new Set<string>();
  let source = "";
  for (const [i, part] of englishText.split(/\{(\w+)\}/).entries()) {
    if (i % 2 === 0) source += escape(part);
    else {
      source += names.has(part) ? `\\k<${part}>` : `(?<${part}>.+)`;
      names.add(part);
    }
  }
  return new RegExp(`^${source}$`, "s");
}

const compile = (rules: Rule[]): Compiled[] => rules.map((r) => ({ ...r, pattern: pattern(english[r.key]) }));

let tables:
  { messages: Compiled[]; limits: Compiled[]; owners: Compiled[]; ownerAmounts: Compiled[]; toolChoices: Compiled[]; timeoutStages: Compiled[] } | undefined;
const compiled = () =>
  (tables ??= {
    messages: compile(messages()),
    limits: compile(limits()),
    owners: compile(owners()),
    ownerAmounts: compile(ownerAmounts()),
    toolChoices: compile(toolChoices()),
    timeoutStages: compile(timeoutStages()),
  });

const codeSpans = /^`[^`]+`( `[^`]+`)*$/;

const converters: Record<Convert, (value: string) => string | undefined> = {
  limit: (value) => apply(compiled().limits, value),
  who: (value) => apply(compiled().owners, value),
  whoList: (value) => {
    const parts = value.split(", ").map((part) => apply(compiled().ownerAmounts, part));
    return parts.every((p) => p !== undefined) ? new Intl.ListFormat(locale(), { type: "conjunction" }).format(parts as string[]) : undefined;
  },
  codeList: (value) => {
    const parts = value.split(", ");
    if (!parts.every((p) => codeSpans.test(p))) return undefined;
    const joined = new Intl.ListFormat(locale(), { type: "conjunction" }).format(parts);
    return joined.replace(/([\u4e00-\u9fff])`/g, "$1 `").replace(/`([\u4e00-\u9fff])/g, "` $1");
  },
  toolChoice: (value) => apply(compiled().toolChoices, value) ?? (codeSpans.test(value) ? value : undefined),
  timeoutStage: (value) => apply(compiled().timeoutStages, value),
  number: (value) => (/^\d+(\.\d+)?$/.test(value) ? formatNumber(Number(value), { maximumFractionDigits: 9 }) : value),
  date: (value) => (Number.isNaN(Date.parse(value)) ? value : formatDate(value, { time: true })),
};

function apply(rules: Compiled[], text: string): string | undefined {
  for (const r of rules) {
    const match = r.pattern.exec(text);
    if (!match) continue;
    const values: Values = {};
    let whole = true;
    for (const [name, value] of Object.entries(match.groups ?? {})) {
      const how = r.convert?.[name];
      const converted = how ? converters[how](value) : value;
      if (converted === undefined) whole = false;
      else values[name] = converted;
    }
    if (whole) return r.text(values);
  }
  return undefined;
}

export function translateMessage(message: string): string {
  return apply(compiled().messages, message) ?? message;
}

export function knownMessage(message: string): string | undefined {
  return apply(compiled().messages, message);
}

export function rules(): { key: string; text: (values: Values) => string }[] {
  const all = compiled();
  return [...all.messages, ...all.limits, ...all.owners, ...all.ownerAmounts, ...all.toolChoices, ...all.timeoutStages];
}
