---
title: Sign-in and single sign-on
description: Choose how people sign in, run an install on single sign-on alone, and get back in when nothing else works.
order: 7
---

People sign in to the console with a password, with a sign-in link sent by email, or through their organization's identity provider (single sign-on with OpenID Connect). Three [settings](configuration.md#sign-in) turn each method on or off for the whole install: `KAESO_AUTH_PASSWORD`, `KAESO_AUTH_MAGIC_LINK` and `KAESO_AUTH_OIDC`. An organization admin connects the identity provider under **Security**; see [single sign-on](../console/organizations-and-projects.md#single-sign-on).

## The sign-in page

The sign-in page usually asks for the email address first, then offers what works for it: a password, a sign-in link, and single sign-on when the address's domain belongs to an organization with an identity provider.

It skips the address and shows the provider's button right away in two cases:

| Case | What the page shows |
| --- | --- |
| Passwords and sign-in links are off, and exactly one organization has an identity provider | That provider's button |
| Someone opens an organization's sign-in link, `https://kaeso.example.com/sign-in?org=org_…` | That organization's button, and **Sign in with your email instead** while other methods are on |

The organization's **Security** page shows its sign-in link. The button says **Continue with** and the name the admin gave the provider, such as **Continue with Acme ID**, or **Continue with SSO** without one. With several organizations on single sign-on and no sign-in link, the page asks for the address and picks the organization by its domain.

**Start without a click**, an option of each connection, sends people to the provider as soon as the page offers only that provider, and right after they enter an address whose only way in is that provider. The page still waits for a click after someone signs out, so the provider's own session doesn't sign them in again at once, and after the provider refused a sign-in, so the reason stays readable.

## Links that go straight to the provider

A link to `GET /platform/v1/auth/oidc/start` sends the browser straight to the identity provider, for a bookmark or an intranet page:

```text
https://kaeso.example.com/platform/v1/auth/oidc/start?org=org_01J9ZKX3M7Q2V8R4T6Y0B1C5D&next=/settings/account
```

`org` names the organization and may be left out while exactly one organization uses single sign-on. The link begins a sign-in only in the browser that opens it: the gateway binds the flow to that browser with a cookie and checks its state, nonce and PKCE verifier when the provider sends the browser back. Whoever signs in at the provider is who ends up signed in, so a link planted on another site can't sign anyone in to someone else's account, and the link needs no `Origin` header. Any problem ends on the sign-in page with the reason.

## Coming back to the right page

Someone who opens a console page without a session lands on the sign-in page with that page in `next`, and returns to it after signing in, through the identity provider too. The gateway keeps `next` with the sign-in flow, never in the provider's URL. Only a path of the console on this install counts: a value that starts with `//` or a backslash, names another site, holds line breaks, or points into the Platform API or the model API is ignored, and the start page opens instead. When a sign-in fails, at the provider or afterwards, the sign-in page keeps `next` for the next attempt. It also keeps the organization when the sign-in began from its sign-in link or its button, so the page shows that organization's button again beside the reason.

## Who signs in through single sign-on

- Only addresses at the connection's email domains. Each domain belongs to one organization on an install.
- Only addresses the provider doesn't mark as unverified.
- Someone new to Kaeso joins the organization with its role for people without an invitation, or with an invitation to their address; without either, they get no access.
- An existing account signs in through single sign-on only where it already is a member. To accept an invitation, it signs in another way first.
- An address at another domain, such as a public mail provider's, can't sign in through the organization's provider, not even with an invitation. On an install with passwords and sign-in links off, such a person has no way in. Don't add the public mail provider's domain to the connection to work around this: every address at that domain would then sign in through your provider, and no other organization could use the domain.

## An install without passwords

To run on single sign-on alone, turn the other methods off:

```yaml
services:
  gateway:
    environment:
      KAESO_AUTH_PASSWORD: "false"
      KAESO_AUTH_MAGIC_LINK: "false"
```

The setup page then asks for no password, and the first admin never gets one. After setup, it opens the organization's **Security** page so the identity provider can be connected in the same session. Give the first admin an address at one of the connection's domains, so they can sign in through it afterwards.

Leave **Require single sign-on** off until the connection works: on such an install it adds nothing, and an organization that requires single sign-on refuses every other session, including one from an operator's sign-in link.

## Getting back in

When no way in works, because the identity provider is down or misconfigured, or the setup session ended before single sign-on worked, print a one-time sign-in link for an existing account on the server:

```bash
docker compose exec gateway kaeso user sign-in-link owner@example.com
```

The link works once, within 15 minutes, even while sign-in links by email are off, so passwords never need to be turned back on. Anyone who has it can sign in as that person: it is only printed in the terminal, never logged or sent, and the audit log records it as `user.sign_in_link`. A deactivated account gets no link; reactivate it first.
