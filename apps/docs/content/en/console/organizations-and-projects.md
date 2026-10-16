---
title: Organizations and projects
description: Create and switch organizations and projects, and read their usage.
order: 1
---

An **organization** holds members, provider keys and projects. A **project** holds a model catalogue, API keys, limits and request logs. People belong to an organization and to some of its projects, each with a role.

## Organizations

Create one from the menu at the foot of the sidebar with **Create organization**. You become its owner, and it starts with a default project. Whether you may create organizations is up to the install; see [instance administration](instance-administration.md#settings).

The same menu lists your organizations and switches between them.

### Overview

The organization's **Overview** shows requests, tokens, cost and errors for a period you choose, from today to all time, the cost by project, and a breakdown by project, model or member. Usage needs the right **View org usage**, and cost **View org costs**.

### Settings

Under **Organization settings**, in the menu at the foot of the sidebar, the organization has these pages:

| Page | What it holds |
| --- | --- |
| General | Picture, name and id, transferring ownership, deleting the organization |
| Members | Members, invitations and roles; see [members and roles](members-and-roles.md) |
| Security | Single sign-on, required single sign-on and two-factor authentication, the maximum key lifetime |
| Platform keys | Keys for scripts that manage the organization; see [API keys](api-keys.md#platform-keys) |
| Logging | Content logging, retention, storage and log deletion; see [request logs](request-logs.md#retention) |
| Audit log | Every change in the organization, with who made it and when; it can be exported as JSON Lines |

### Single sign-on

Under **Security**, choose **Connect an identity provider** to sign people in through your OpenID Connect provider. Enter the issuer URL, the client id and secret of the app you registered there, and the email domains that sign in with it. Choose whether people can join without an invitation, and with which role. The page shows the redirect URL to register with the provider.

Once connected, **Require single sign-on** makes it the only way into the organization. **Require two-factor authentication** applies to password sign-in, not to sign-in links or single sign-on.

### Deleting an organization

If the install allows it, an owner deletes the organization under **General** › **Delete organization**, typing its name to confirm; it asks for a recent sign-in, and members are told by email. Its keys stop working at once; it stays read-only and restorable for a grace period, 7 days by default, and is then deleted for good. Until then, every page shows a banner, and **Restore organization** brings it back.

## Projects

**Projects** lists the organization's projects; archived ones are under **All**. **Create project** asks for a name; you become the project's admin. Creating projects needs the right **Create projects**.

The **default project** is the one every member joins and new members start in. To change it, choose **Make default** in another project's menu; this needs the right **Edit organization**.

### Overview

A project's **Overview** shows its requests, tokens, cost and errors over a period, a breakdown by time, model, key or member, its limits with what is used, and recent errors.

### Settings

| Setting | What it does |
| --- | --- |
| Picture, name | Shown in the sidebar and in request logs |
| Key lifetime | **Use the organization's limit**, or a shorter maximum for new keys of this project |
| Content logging | Whether requests and replies are kept with the log entry; it can't be on when the organization has it off |
| Archive project | Archives the project with its keys and service accounts, permanently, so its keys stop working; the default project can't be archived |

Projects are archived, never deleted, so their usage and logs stay.
