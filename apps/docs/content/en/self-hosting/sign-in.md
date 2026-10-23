---
title: Sign-in and single sign-on
description: Choose how people sign in, run an install without passwords, and get back in when nothing else works.
order: 7
---

People sign in to the console with a password, with a sign-in link sent by email, or through their organization's identity provider over OpenID Connect. Two [sign-in settings](configuration.md#sign-in) turn passwords and sign-in links on or off for the whole install. Single sign-on is set up per organization, not for the install: it works for an organization once its admin connects the identity provider on the organization's **Security** page ([organizations and projects](../console/organizations-and-projects.md)), and every organization can.

## The sign-in page

The sign-in page always asks for the email address first. It then offers what works for that address: a password, a sign-in link, and single sign-on when the address is at a verified email domain of an organization with an identity provider ([verify your email domains](../console/organizations-and-projects.md#verify-your-email-domains)). An address at any other domain, or at a domain its organization hasn't verified yet, is never offered single sign-on.

The button says **Continue with** and the name the admin gave the provider, or **Continue with SSO** without one. When single sign-on is the address's only way in and the connection has **Start without a click** on, the page goes on to the provider right after the address, without the button. After someone signs out, or when the provider refused a sign-in, the page asks for the address again and shows the reason.

Single sign-on is offered only after the address: no link starts it for an organization, so a link on another site can't send a browser through an identity provider into someone else's account. For a bookmark or an intranet page, link to the sign-in page, such as `https://kaeso.example.com/sign-in`; with **Start without a click** on, members go on to the provider as soon as they enter their address.

Single sign-on never replaces another account's session either. When the provider answers with another account while the browser is signed in, the browser stays signed in and the page says `signed_in_as_other`; to switch accounts, sign out first.

After signing in, people return to the console page they first opened, through the identity provider too.

## Recent sign-ins

When an action needs a recent sign-in, the console asks the provider to sign the person in again (`prompt=login`, `max_age=0`), as OpenID Connect specifies. Only an answer that says the person signed in during the confirmation counts, so a confirmation always means a fresh sign-in.

A provider can also sign someone in with a session of its own from earlier, without asking. So a session from single sign-on dates from the provider's `auth_time`, not from when it came back to Kaeso, and counts as a recent sign-in only while that time is within 15 minutes. Kaeso measures it against the token's issue time `iat`, on the provider's clock, so the two servers' clocks needn't agree. Without `auth_time`, the session counts as older than 15 minutes from the start.

Some providers can't confirm a recent sign-in: they leave `auth_time` out, or ignore `max_age=0` and answer with an earlier sign-in of their own. OpenID Connect requires a fresh sign-in and `auth_time` once `max_age` is sent, but Kaeso can't tell a provider that ignores this apart until a confirmation fails. So the console offers single sign-on to confirm only when the account has no other way: no password, no sign-in link and no link to set a password.

A confirmation without a fresh sign-in is refused with `cannot_confirm`, and Kaeso notes on the connection that the provider can't confirm. The console then stops offering it, and the organization's **Security** page shows **Recent sign-in**. For an account whose organization requires single sign-on ([managed accounts](#managed-accounts)), nothing else confirms, so it can't take actions that need a recent sign-in. Kaeso forgets the note when the issuer or client id changes; other sign-ins note nothing, since without `max_age` a provider may reuse its sign-in.

## Who signs in through single sign-on

- Only addresses at the connection's verified email domains ([verify your email domains](../console/organizations-and-projects.md#verify-your-email-domains)). One organization on an install verifies each domain.
- Only addresses the provider marks as verified with `email_verified`. For a provider that never sends the claim, a connection can turn off **Require verified addresses**; an address marked unverified is still refused.
- Someone new to Kaeso joins the organization with an invitation to their address, or with the connection's role for people without one. Without either, they get no access. Kaeso creates their account even where the [account creation setting](../console/instance-administration.md#change-install-settings) allows no sign-ups, because the organization's provider vouches for them.
- Kaeso links each account to its identity at the provider, the provider's issuer and subject. Later sign-ins follow that link, not the email address.
- An existing account signs in through single sign-on only once its owner links it, from **Account** › **Security** after signing in another way ([link single sign-on](../console/account.md#link-single-sign-on)), and only where it is a member. To accept an invitation, it signs in another way first.
- An address at another domain, such as a public mail provider's, can't sign in through the organization's provider, not even with an invitation.

What a sign-in through a provider opens depends on whether its organization manages the account ([managed accounts](#managed-accounts)).

A connection can't list a public mail provider's domain, such as `gmail.com`: every address there would sign in through one organization's provider.

## Managed accounts

An organization with single sign-on manages the accounts at its verified email domains. Kaeso works this out from the account's address on every request, so a new address, a domain that loses its verification or a removed connection changes it at once. One organization verifies each domain, so at most one organization manages an account. The organization's other members are guests: they belong to it, but their address is at another domain.

A managed account's sign-in through its organization's provider is a full sign-in, as when a work account belongs to one company. It opens every organization the person belongs to, each with its own requirements, and changes to the account. A sign-in through the provider of an organization that doesn't manage the account opens only that organization. That happens when the address is at another domain, such as after it changed, or when the domain is no longer verified.

| | Managed account, through its organization's provider | Through another organization's provider | Password or sign-in link |
| --- | --- | --- | --- |
| Organizations | Every one the person belongs to | Only the provider's organization | Every one, except one that manages the account and requires single sign-on |
| An organization's two-factor requirement | Met by the provider | Met in the provider's organization | Needs a code |
| Account changes | Allowed, with a recent sign-in | Need a password or a sign-in link | Allowed, with a recent sign-in |
| Instance administration | Only through the organization chosen for instance admins | Needs a password or a sign-in link | Allowed for instance admins |

The provider owns a managed account's address. `POST me/email` refuses it with `Your organization manages this address; change it at its identity provider.`, and a link to confirm an earlier change no longer works. When a sign-in through the provider brings another address at one of the organization's verified domains, Kaeso changes the account's address to it, since the issuer and subject link the account, not the address. The organization's audit log records it as `user.email_change`. An address that another account holds is never taken, and the provider of an organization that doesn't manage the account never changes it.

**Account** › **Profile** shows **Managed by** and the organization's name, and an organization's **Members** page marks guests ([after signing in through single sign-on](../console/account.md#after-signing-in-through-single-sign-on)).

### Require single sign-on

With **Require single sign-on** on, the accounts an organization manages sign in only through its provider, anywhere on the install. The sign-in page offers them the provider alone, and a password, a sign-in link, a sign-up or an invitation without an account is refused with `Your organization requires single sign-on; continue with its identity provider.` Turning the requirement on ends their sessions from a password or a sign-in link, except the session that turns it on, and so does verifying another domain while it is on.

Guests keep their own sign-in and open the organization with it, under its two-factor requirement. A sign-in link from the server still signs a managed account in ([get back in](#get-back-in)).

### Instance admins

No organization's identity provider administers the instance by default, so an organization's provider admin can't make themselves the instance's admin. To let one, choose the organization under **Instance** › **General** › **Single sign-on for instance admins** ([change install settings](../console/instance-administration.md#change-install-settings)). An instance admin's sign-in through that organization's provider then administers the instance, while the organization manages their account.

Choosing it needs an instance admin's recent sign-in with a password or a sign-in link, never one through single sign-on, and the instance's audit log records it as `instance.admin_sso_update`. Only an organization with a connection can be chosen. If it later loses its connection, the setting has no effect, and **General** says so.

## Run without passwords or sign-in links

To sign in only through organizations' identity providers, turn passwords and sign-in links off before the first start:

```yaml title="compose.override.yaml"
services:
  gateway:
    environment:
      KAESO_AUTH_PASSWORD: "false"
      KAESO_AUTH_MAGIC_LINK: "false"
```

1. Give the first admin an address at a domain the identity provider will cover, so they can sign in through it later.
2. Open the setup link. The setup page asks for no password, and then opens the organization's **Security** page.
3. Connect the identity provider in the same session.
4. Under **Account** › **Security**, choose **Link** next to the organization, and sign in at the provider. That links your account and checks the connection works.

The sign-in page still asks for the email address first and offers the provider of its organization. An address at no organization's verified domain has no way in, and the page says so.

A sign-in through single sign-on administers the instance only once an instance admin chooses the organization for it ([instance admins](#instance-admins)). Until then, sign in for instance administration with a [one-time sign-in link](#get-back-in) from the server.

> [!IMPORTANT]
> Leave **Require single sign-on** off until the connection works. On an install without passwords it adds nothing, and it closes the organization to every session of the accounts it manages that didn't come from its provider, including one from an operator's sign-in link.

## Get back in

When no way in works, print a one-time sign-in link for an existing account on the server. This helps when the identity provider is down or misconfigured, or the setup session ended before single sign-on worked:

```bash
docker compose exec gateway kaeso user sign-in-link owner@example.com
```

The link works once, within 15 minutes, even while sign-in links by email are off, so you never need to turn passwords back on. Anyone who has the link can sign in as that person. Kaeso only prints it in your terminal, never logs or sends it, and the audit log records it as `user.sign_in_link`. A deactivated account gets no link until you reactivate it. With two-factor authentication on, the link also asks for a code; when the person has lost their authenticator, `kaeso user reset-2fa` removes it first.

The link signs in a managed account even while its organization requires single sign-on. That session opens the person's account, their other organizations and, for an instance admin, instance administration; the organization that requires single sign-on stays closed to it.

## Next steps

- [Instance administration](../console/instance-administration.md): Manage users, organizations and the instance's settings.
- [Troubleshooting](troubleshooting.md): Fix common problems with an install.
