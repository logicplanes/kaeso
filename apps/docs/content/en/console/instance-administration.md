---
title: Instance administration
description: Manage the users, organizations and settings of the whole install.
order: 10
---

**Instance admins** run the install. The first user, created with the setup link, is one, and can make others instance admins. Being an instance admin gives no rights inside organizations: it shows their metadata, never their keys, logs or content.

The pages are under **Instance**, in the menu at the foot of the sidebar.

## Users

**Users** lists every account on the install, active, deactivated or deleted. **Invite user** invites someone to the install without an organization; they join one when an organization invites them. A user's sheet has these actions:

| Action | What it does |
| --- | --- |
| Instance admin | Makes the user an instance admin, or no longer one |
| Deactivate, Reactivate | Blocks sign-in, ends their sessions and disables their keys, or allows them again |
| End sessions | Signs the user out everywhere |
| Reset two-factor | Removes their authenticator app and recovery codes, for someone who lost both |
| Delete user | Removes their name and email address and archives their keys; history shows a deleted user |

The affected user gets an email when their sessions end or their two-factor authentication is reset.

## Organizations

**Organizations** lists every organization with its members, projects and state. **Create organization** creates one with the owner you choose, or with you as owner when you choose none.

An organization's page shows its metadata, owners, members and projects, and its requests, tokens, cost and stored logs of the last 30 days. **Add owner** makes any active user an owner, you included (**Add me**); that is how an instance admin gets into an organization, and the organization's audit log records it. The page also renames, deletes and restores the organization, and **Delete now** deletes a scheduled organization at once instead of after its grace period.

## Settings

| Setting | Choices |
| --- | --- |
| Account creation | **Invitation only**, **Allowed email domains** or **Anyone**; invited people can always create their account |
| Organization creation | **Instance admins only**, **Users on allowed domains** or **Any signed-in user** |
| Allowed domains | The domains for the two settings above; `*.example.com` covers subdomains |
| Deleting organizations | Whether organizations can be deleted at all, and the grace period, 1 to 90 days |
| Request log retention | The defaults for new organizations, and the longest an organization may choose |

Changing account creation, organization creation or the allowed domains needs a recent sign-in. Sign-in methods and email are set in the gateway's [configuration](../self-hosting/configuration.md) and shown here read-only; **Send test email** checks that the gateway can send mail.

## Model sources

**Model sources** shows the install's models.dev data: the data in use, the last sync, and the automatic sync, every 24 hours by default. **Sync now** fetches it at once; if models.dev can't be reached, the current data stays in use.

## System

**System** shows the gateway's version and state, whether PostgreSQL, ClickHouse and Valkey are up, the request log buffer and the storage. It checks every 10 seconds, and **Check now** checks at once.

## Updates

The version the install runs, the newest Kaeso release with the notes of every release since, and when the gateway last looked for one; a **New** mark beside **Updates** in the settings sidebar says a newer release is out. **Check now** looks at once. The page's settings turn the twice-daily check off, offer pre-releases, and turn on one-click updates through the [updater](../self-hosting/upgrades.md#one-click-updates). With the updater, **Update to …** updates the install after a confirmation and follows each step until Kaeso is back; without it, or for a release that needs steps by hand, the page shows the commands and the upgrade guide. The history lists every update with who started it and how it ended.

## Audit log

The instance's **Audit log** records changes to users, organizations and the install's settings, with who made them and when.
