---
title: Instance administration
description: Manage the users, organizations and settings of the whole install.
order: 10
---

**Instance admins** run the install. The first user, created through the setup link, is one; they can make others instance admins. Being an instance admin gives no rights inside organizations: it shows their metadata, never their keys, logs or content.

The pages are under **Settings** › **Instance**: choose **Instance** in the menu at the foot of the sidebar.

## Users

Every account on the install, active, deactivated or deleted. **Invite user** invites someone to the install without an organization. A user's sheet:

| Action | What it does |
| --- | --- |
| Instance admin | Makes the user an instance admin, or stops it |
| Deactivate, Reactivate | Blocks sign-in, ends their sessions and disables their keys, or allows them again |
| End sessions | Signs the user out everywhere |
| Reset two-factor | Removes their authenticator and recovery codes, for someone who lost both |
| Delete user | Removes their name and email and archives their keys; history shows them as a deleted user |

## Organizations

Every organization, with its members, projects and state. **Create organization** creates one with an owner you choose, or with you as owner when you choose none.

An organization's page shows its metadata, owners, and the last 30 days of requests, tokens, cost and logs. **Add owner** makes any active user an owner, including you; that is how an instance admin gets into an organization, and it is recorded in the organization's audit log. From here you can also rename, delete and restore organizations, or delete one at once instead of after its grace period.

## Settings

| Setting | Choices |
| --- | --- |
| Account creation | Invitation only; allowed email domains; anyone |
| Organization creation | Instance admins only; users on allowed domains; any signed-in user |
| Allowed email domains | Domains for the two settings above; `*.example.com` covers subdomains |
| Deleting organizations | Whether organizations can be deleted, and the grace period, 1 to 90 days |
| Request log retention | Defaults for new organizations, and the longest an organization may choose |

Sign-in methods and email are set in the gateway's [configuration](../self-hosting/configuration.md) and shown here read-only. **Send test email** checks the email settings.

## Model sources

The state of the install's model data from models.dev: the data in use, the last sync and the automatic sync, every 24 hours by default. **Sync now** refreshes it; if models.dev can't be fetched, the current data stays in use.

## System

The gateway's version and state, whether PostgreSQL, ClickHouse and Valkey are up, the request log buffer and storage. The page checks every 10 seconds.

## Audit log

Changes to users, organizations and the install's settings, with who made them and when.
