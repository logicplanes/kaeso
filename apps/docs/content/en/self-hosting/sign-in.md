---
title: Sign-in and single sign-on
description: Choose how people sign in, run an install on single sign-on alone, and get back in when nothing else works.
order: 7
---

People sign in to the console with a password, with a sign-in link sent by email, or through their organization's identity provider over OpenID Connect. Three [sign-in settings](configuration.md#sign-in) turn each method on or off for the whole install. An organization admin connects the identity provider on the organization's **Security** page ([organizations and projects](../console/organizations-and-projects.md)).

## The sign-in page

The sign-in page asks for the email address first. It then offers what works for that address: a password, a sign-in link, and single sign-on when the address's domain belongs to an organization with an identity provider.

The page shows a provider's button right away in two cases:

- Passwords and sign-in links are off, and exactly one organization has an identity provider.
- Someone opens the organization's sign-in link, such as `https://kaeso.example.com/sign-in?org=org_…`, which its **Security** page shows. While other methods are on, the page also offers **Sign in with your email instead**.

The button says **Continue with** and the name the admin gave the provider, or **Continue with SSO** without one. With **Start without a click** on, the page sends people to the provider at once when it is their only way in. It still waits for a click after someone signs out, or when the provider refused a sign-in, so the reason stays readable.

After signing in, people return to the console page they first opened, through the identity provider too.

## Links that go straight to the provider

A link to `/platform/v1/auth/oidc/start` sends the browser straight to the identity provider, for a bookmark or an intranet page:

```text
https://kaeso.example.com/platform/v1/auth/oidc/start?org=org_01J9ZKX3M7Q2V8R4T6Y0B1C5D&next=/settings/account
```

`org` names the organization, and you can leave it out while only one organization uses single sign-on. `next` is the console page to open afterwards. The link signs in only the browser that opens it, as whoever signs in at the provider. So a link planted on another site can't sign anyone in to someone else's account.

## Who signs in through single sign-on

- Only addresses at the connection's email domains. Each domain belongs to one organization on an install.
- Only addresses the provider doesn't mark as unverified.
- Someone new to Kaeso joins the organization with an invitation to their address, or with the connection's role for people without one. Without either, they get no access.
- An existing account signs in through single sign-on only where it already is a member. To accept an invitation, it signs in another way first.
- An address at another domain, such as a public mail provider's, can't sign in through the organization's provider, not even with an invitation.

Don't add a public mail provider's domain to a connection to let such people in. Every address at that domain would then sign in through your provider, and no other organization could use the domain.

## Run on single sign-on alone

To sign in only through identity providers, turn the other methods off before the first start:

```yaml title="compose.override.yaml"
services:
  gateway:
    environment:
      KAESO_AUTH_PASSWORD: "false"
      KAESO_AUTH_MAGIC_LINK: "false"
```

1. Give the first admin an address at a domain the identity provider will cover, so they can sign in through it later.
2. Open the setup link. The setup page asks for no password, and then opens the organization's **Security** page.
3. Connect the identity provider in the same session, and sign in through it once to check it works.

> [!IMPORTANT]
> Leave **Require single sign-on** off until the connection works. On an install without passwords it adds nothing, and it refuses every other session, including one from an operator's sign-in link.

## Get back in

When no way in works, print a one-time sign-in link for an existing account on the server. This helps when the identity provider is down or misconfigured, or the setup session ended before single sign-on worked:

```bash
docker compose exec gateway kaeso user sign-in-link owner@example.com
```

The link works once, within 15 minutes, even while sign-in links by email are off, so you never need to turn passwords back on. Anyone who has the link can sign in as that person. Kaeso only prints it in your terminal, never logs or sends it, and the audit log records it as `user.sign_in_link`. A deactivated account gets no link until you reactivate it.

## Next steps

- [Instance administration](../console/instance-administration.md): Manage users, organizations and the instance's settings.
- [Troubleshooting](troubleshooting.md): Fix common problems with an install.
