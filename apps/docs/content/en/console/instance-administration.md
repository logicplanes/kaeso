---
title: Instance administration
description: Manage the users, organizations, settings and updates of the whole install.
order: 11
---

Instance admins run the install. The first user, created with the setup link, is one, and can make others instance admins. Being an instance admin gives no rights inside organizations: you see their metadata, never their keys, logs or content.

The pages are under **Instance**, in the menu at the foot of the sidebar.

## Manage users

**Users** lists every account on the install: active, deactivated or deleted. **Invite user** invites someone to the install without an organization. They join one when an organization invites them.

Instance admins carry an **Instance admin** badge, and **Managed by** names the organization that manages an account ([managed accounts](../self-hosting/sign-in.md#managed-accounts)). Choose a user to open their sheet, which lists the organizations they belong to, with their role in each, and has these actions:

| Action | What it does |
| --- | --- |
| Instance admin | Makes the user an instance admin, or no longer one |
| Deactivate, Reactivate | Blocks sign-in, ends their sessions and disables their keys, or allows them again |
| End sessions | Signs the user out everywhere and revokes their agents |
| Reset two-factor | Removes their authenticator app and recovery codes, for someone who lost both; also ends their sessions and agents |
| Delete user | Removes their name and email address and archives their keys. History shows a deleted user |

Kaeso emails the user when you end their sessions or reset their two-factor authentication.

## Manage organizations

**Organizations** lists every organization with its members, projects and state. **Create organization** creates one with the owner you choose, or with you as its owner when you choose none.

An organization's page shows its metadata, owners, members and projects. It also shows its requests, tokens, cost and stored logs for the last 30 days. On it, you can rename, delete and restore the organization.

To get into an organization, choose **Add owner** and pick any active user, or **Add me** for yourself. The organization's audit log records it. For an organization scheduled for deletion, **Delete now** deletes it at once instead of after its grace period.

## Change install settings

Under **General**:

| Setting | Choices |
| --- | --- |
| Account creation | **Invitation only**, **Allowed email domains** or **Anyone**. Invited people can always create their account |
| Organization creation | **Instance admins only**, **Users on allowed domains** or **Any signed-in user** |
| Allowed domains | The domains for the two settings above. `*.example.com` covers subdomains |
| Single sign-on for instance admins | **None**, or an organization with single sign-on whose provider may sign its managed instance admins in to administer the instance ([instance admins](../self-hosting/sign-in.md#instance-admins)) |
| Time zone | The install's time zone. Every time in the console shows in it, for everyone, and automatic updates follow it. **Use this zone** switches to your browser's |
| Deleting organizations | Whether organizations can be deleted at all, and the grace period, 1 to 90 days |

Under **Logging**, **Request log retention** sets the defaults for new organizations, and the longest an organization may choose.

Changing account creation, organization creation or the allowed domains needs a recent sign-in. Changing single sign-on for instance admins needs one with a password or a sign-in link; without one, an operator changes it on the server with `kaeso instance admin-sso` ([run Kaeso's commands](../self-hosting/installation.md#run-kaesos-commands)). The setup page starts the time zone at the first user's, and usage charts keep counting by UTC day.

Passwords, sign-in links and email are set in the gateway's [configuration](../self-hosting/configuration.md). **General** shows whether passwords and sign-in links are on and **System** shows email, both read-only. Single sign-on is set up per organization, on its **Security** page. **Send test email** on **System** checks that the gateway can send mail.

## Let agents in

Agents, such as Claude Code or Codex, act for their user through the [MCP server](../api/mcp.md). Under **General**, **Agents** holds the instance's agent policy:

| Setting | What it does |
| --- | --- |
| Allow agents | On by default. Turned off, no agent can sign in, and every agent gets 403 at its next request |
| Most on the instance | **None**, **Read**, **Configure** or **Admin**, for the instance rights an instance admin can give their agents. **Configure** is the default |

Only an instance admin's console session changes the policy; no agent can. An agent never makes someone an instance admin, changes single sign-on for instance admins, or deletes a user or an organization at once. [Instance rights](../concepts/rights.md#instance-rights) lists what each right allows.

## Sync model data

**Model sources** shows the install's models.dev data: the data in use, the last sync, and the automatic sync, every 24 hours by default. **Sync now** fetches it at once. If models.dev can't be reached, the current data stays in use.

## Check the system

**System** shows the gateway's version and state, and whether PostgreSQL, ClickHouse and Valkey are up. It also shows the request log buffer, the storage and how the gateway sends email. The page refreshes every 10 seconds.

## Update Kaeso

**Updates** shows the version the install runs and the newest Kaeso release, with the notes of every release since. A **New** mark beside **Updates** in the settings sidebar says that a newer release is out. The gateway looks for releases every 5 minutes, and the page says when a check failed.

- **Releases** offers stable releases only, or pre-releases too. Only the gateway's [configuration](../self-hosting/configuration.md) turns the check off, for installs without internet access.
- With the [updater](../self-hosting/upgrades.md#the-updater), **How updates install** chooses **Manually** or **Automatically**. Automatically, Kaeso installs new releases in an update window: daily or weekly, at an hour in the install's time zone. A new major version, or a release with steps by hand, waits for you.
- **Update to …** updates the install after you confirm, in either mode, and follows each step until Kaeso is back.
- Without the updater, or for a release that needs steps by hand, the page shows the commands and the upgrade guide.

The history lists every update, with who started it, or **Automatically**, and how it ended.

After an update, everyone who opens the console sees **What's new in Kaeso** once. It shows the new version on its own artwork, which never changes, with its improvements, and **Next** leads through every release they missed. **Continue**, Escape or a click outside closes it. **See what's new** on **Updates** shows the running version's notes again.

## Read the audit log

The instance's **Audit log** records changes to users, organizations and the install's settings, with who made each change and when. It marks changes by agents as an organization's audit log does, and **Done by agents** shows only those ([read the audit log](organizations-and-projects.md#read-the-audit-log)).

## Next steps

- [Configuration](../self-hosting/configuration.md): Set the sign-in methods, email and other settings of the gateway.
- [Upgrades](../self-hosting/upgrades.md): Move the install to a new release, with or without the updater.
- [Sign-in and single sign-on](../self-hosting/sign-in.md): Get back in when nobody can sign in.
