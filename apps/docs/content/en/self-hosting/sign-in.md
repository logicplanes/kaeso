---
title: Sign-in and single sign-on
description: Choose how people sign in, run an install without passwords, and get back in when nothing else works.
order: 7
---

People sign in to the console with a password, with a sign-in link sent by email, or through their organization's identity provider over OpenID Connect. Two [sign-in settings](configuration.md#sign-in) turn passwords and sign-in links on or off for the whole install. Single sign-on is set up per organization, not for the install: it works for an organization once its admin connects the identity provider on the organization's **Security** page ([organizations and projects](../console/organizations-and-projects.md)), and every organization can.

## The sign-in page

The sign-in page always asks for the email address first. It then offers what works for that address: a password, a sign-in link, and single sign-on when the address is at a verified email domain of an organization with an identity provider ([verify your email domains](../console/organizations-and-projects.md#verify-your-email-domains)). An address at any other domain, or at a domain its organization hasn't verified yet, is never offered single sign-on.

The button says **Continue with** and the name the admin gave the provider, or **Continue with SSO** without one. When single sign-on is the address's only way in and the connection has **Start without a click** on, the page goes on to the provider right after the address, without the button. After someone signs out, or when the provider refused a sign-in, the page asks for the address again and shows the reason.

The only way to single sign-on without entering an email is the organization's own start link, and only while its connection has **Start without a click** on ([links that go straight to the provider](#links-that-go-straight-to-the-provider)).

After signing in, people return to the console page they first opened, through the identity provider too.

When an action needs a recent sign-in, the console asks the provider to sign the person in again (`prompt=login`, `max_age=0`), as OpenID Connect specifies. A provider whose answer says the last sign-in is older than the request, or doesn't say when it was (`auth_time`), is refused with `stale_sign_in`, so a confirmation always means a fresh sign-in.

A provider can also sign someone in with a session of its own from earlier, without asking. So a session from single sign-on dates from the provider's `auth_time`, not from when it came back to Kaeso, and counts as a recent sign-in only while that time is within 15 minutes. Kaeso measures it against the token's issue time `iat`, on the provider's clock, so the two servers' clocks needn't agree. Without `auth_time`, the session counts as older than 15 minutes from the start.

## Links that go straight to the provider

An organization's start link, the **Sign-in link** on its **Security** page, sends a browser that isn't signed in straight to the identity provider, for a bookmark or an intranet page:

```text
https://kaeso.example.com/platform/v1/auth/oidc/start?org=org_01J9ZKX3M7Q2V8R4T6Y0B1C5D&next=/settings/account
```

`org` names the organization, and `next` is the console page to open afterwards. While the organization's connection has **Start without a click** on, the link opens the organization's sign-in page, which names the organization and its provider and goes on to the provider at once. With it off, the link opens the sign-in page, which asks for the email address first and keeps `next`.

When the browser is signed in to Kaeso already, the page asks first: it names the account that is signed in and waits for a click. Single sign-on never replaces another account's session. When the provider answers with another account, the browser stays signed in and the page says `signed_in_as_other`; to switch accounts, sign out first. So a link planted on another site can neither sign a browser out nor sign it in to someone else's account.

## Who signs in through single sign-on

- Only addresses at the connection's verified email domains ([verify your email domains](../console/organizations-and-projects.md#verify-your-email-domains)). One organization on an install verifies each domain.
- Only addresses the provider marks as verified with `email_verified`. For a provider that never sends the claim, a connection can turn off **Require verified addresses**; an address marked unverified is still refused.
- Someone new to Kaeso joins the organization with an invitation to their address, or with the connection's role for people without one. Without either, they get no access. Kaeso creates their account even where the [account creation setting](../console/instance-administration.md#change-install-settings) allows no sign-ups, because the organization's provider vouches for them.
- Kaeso links each account to its identity at the provider, the provider's issuer and subject. Later sign-ins follow that link, not the email address.
- An existing account signs in through single sign-on only once its owner links it, from **Account** › **Security** after signing in another way ([link single sign-on](../console/account.md#link-single-sign-on)), and only where it is a member. To accept an invitation, it signs in another way first.
- An address at another domain, such as a public mail provider's, can't sign in through the organization's provider, not even with an invitation.

A sign-in through an organization's provider opens only that organization, and reads the person's own account. Other organizations, instance administration and changes to the account's email, password and two-factor authentication need another sign-in ([after signing in through single sign-on](../console/account.md#after-signing-in-through-single-sign-on)).

A connection can't list a public mail provider's domain, such as `gmail.com`: every address there would sign in through one organization's provider.

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

A sign-in through single sign-on doesn't administer the instance. For instance administration, sign in with a [one-time sign-in link](#get-back-in) from the server.

> [!IMPORTANT]
> Leave **Require single sign-on** off until the connection works. On an install without passwords it adds nothing, and it refuses every other session, including one from an operator's sign-in link.

## Get back in

When no way in works, print a one-time sign-in link for an existing account on the server. This helps when the identity provider is down or misconfigured, or the setup session ended before single sign-on worked:

```bash
docker compose exec gateway kaeso user sign-in-link owner@example.com
```

The link works once, within 15 minutes, even while sign-in links by email are off, so you never need to turn passwords back on. Anyone who has the link can sign in as that person. Kaeso only prints it in your terminal, never logs or sends it, and the audit log records it as `user.sign_in_link`. A deactivated account gets no link until you reactivate it. With two-factor authentication on, the link also asks for a code; when the person has lost their authenticator, `kaeso user reset-2fa` removes it first.

## Next steps

- [Instance administration](../console/instance-administration.md): Manage users, organizations and the instance's settings.
- [Troubleshooting](troubleshooting.md): Fix common problems with an install.
