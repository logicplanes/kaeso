---
title: Sign-in and single sign-on
description: Choose how people sign in, run an install on single sign-on alone, and get back in when nothing else works.
order: 5
---

People sign in to the console with a password, with a sign-in link sent by email, or through their organization's identity provider (single sign-on, OpenID Connect). Three [settings](configuration.md#sign-in) turn each method on or off for the whole install: `KAESO_AUTH_PASSWORD`, `KAESO_AUTH_MAGIC_LINK` and `KAESO_AUTH_OIDC`. An organization admin connects the identity provider under **Security**; see [single sign-on](../console/organizations-and-projects.md#single-sign-on).

## The sign-in page

The sign-in page usually asks for the email address first, then offers what works for it: a password, a sign-in link, and single sign-on when the address's domain belongs to an organization with an identity provider.

It skips the address and offers the provider's button straight away in two cases:

| Case | What the page shows |
| --- | --- |
| The install signs in only with single sign-on (passwords and sign-in links are off) and exactly one organization has an identity provider | That provider's button |
| Someone opens an organization's sign-in link, `https://kaeso.example.com/sign-in?org=org_…` | That organization's button, and **Sign in with your email instead** while other methods are on |

The organization's **Security** page shows its sign-in link. The button says **Continue with** and the name the admin gave the provider, such as **Continue with Acme ID**, or **Continue with SSO** without one. With several organizations on single sign-on and no sign-in link, the page asks for the address, which picks the organization by its domain.

**Start without a click**, an option of each connection, sends people to the provider as soon as the page offers it alone, and right after they enter an address whose only way in is that provider. The page waits for a click after someone signs out, so that they aren't signed in again at once by the provider's own session, and after the provider refused a sign-in, so that the reason stays readable.

## Links that go straight to the provider

A link to `GET /platform/v1/auth/oidc/start` sends the browser straight to the identity provider, for a bookmark or an intranet page:

```text
https://kaeso.example.com/platform/v1/auth/oidc/start?org=org_01J9ZKX3M7Q2V8R4T6Y0B1C5D&next=/settings/account
```

`org` names the organization; it can be left out while exactly one organization uses single sign-on. The link only begins a sign-in in the browser that opens it: the gateway binds the flow to that browser with a cookie, and checks its state, nonce and PKCE verifier when the provider sends the browser back. Whoever signs in at the provider is who ends up signed in, so a link planted on another site can't sign anyone in to someone else's account, and the link needs no `Origin` header. Problems end on the sign-in page with the reason.

## Coming back to the right page

Someone who opens a console page without a session goes to the sign-in page with that page in `next`, and comes back to it after signing in, through the identity provider too. `next` is kept with the sign-in flow on the gateway, never in the provider's URL. Only a path of the console on this install counts: a value that starts with `//` or a backslash, names another site, holds line breaks, or points into the Platform API or the model API is refused, and the start page opens instead. When the provider refuses someone, the sign-in page keeps `next` for the next attempt.

## Who signs in through single sign-on

- Only addresses at the email domains of the connection sign in through it. Each domain belongs to one organization on an install.
- The provider must not mark the address as unverified.
- Someone new to Kaeso joins the organization with its role for people without an invitation, or with an invitation to their address; without either, they get no access.
- An existing account signs in through single sign-on only where it already is a member; to accept an invitation, it signs in another way first.
- An address at another domain, such as one at a public mail provider, can't sign in through the organization's provider, not even with an invitation. On an install with passwords and sign-in links off, such a person has no way in. Don't list a public mail provider's domain to work around it: every address at that domain would then sign in through your provider, and no other organization could use the domain.

## An install without passwords

To run on single sign-on alone, turn the other methods off:

```yaml
services:
  gateway:
    environment:
      KAESO_AUTH_PASSWORD: "false"
      KAESO_AUTH_MAGIC_LINK: "false"
```

The setup page then asks for no password, and the first admin never gets one. After setup it opens the organization's **Security** page to connect the identity provider in the same session. Give the first admin an address at one of the connection's domains, so they can sign in through it afterwards. Requiring single sign-on adds nothing on such an install; leave it off until the connection works, because an organization that requires single sign-on refuses every other session, the one from an operator's sign-in link included.

## Getting back in

When no way in works, because the identity provider is down or misconfigured, or the setup session ended before single sign-on worked, print a one-time sign-in link for an existing account where the gateway runs:

```bash
docker compose exec gateway kaeso user sign-in-link owner@example.com
```

The link works once, within 15 minutes, even while sign-in links by email are off, and needs no password to be turned back on. Anyone who has it can sign in as that person, so it is printed only in the terminal, never logged or sent, and the audit log records it as `user.sign_in_link`.

## The setup link

While an install has no users, the gateway's log says that it isn't set up yet, never the setup link itself: anyone who can read the logs could otherwise take the install over. `kaeso setup-link` prints the link; see [what happens at the first start](installation.md#what-happens-at-the-first-start).
