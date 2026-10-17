---
title: Organizations and projects
description: Create and switch organizations and projects, read their usage, and manage their settings.
order: 1
---

An **organization** holds members, provider keys and projects. A **project** holds a model catalogue, API keys, service accounts, limits and request logs. People belong to an organization with an organization role, and to some of its projects with a project role ([members and roles](members-and-roles.md)).

## Organizations

**Create organization**, in the menu at the foot of the sidebar, makes you the new organization's owner and gives it a default project. The install decides who may create organizations ([instance settings](instance-administration.md#settings)). The same menu lists your organizations and switches between them.

### Overview

The organization's **Overview** shows requests, tokens, cost and errors for a period from today to all time, the cost by project, and a usage breakdown by project, model or member. Usage needs the right **View org usage**, cost **View org costs**.

### Settings

**Organization settings**, in the menu at the foot of the sidebar, has these pages:

| Page | What it holds |
| --- | --- |
| General | Picture, name and organization id; **Transfer ownership**; deleting the organization |
| Members | Members, invitations and roles ([members and roles](members-and-roles.md)) |
| Security | Single sign-on, required single sign-on and two-factor authentication, the maximum key lifetime |
| Platform keys | Keys for scripts that manage the organization ([platform keys](api-keys.md#platform-keys)) |
| Logging | Content logging, retention, storage and log deletion ([request logs](request-logs.md#retention)) |
| Audit log | Every change in the organization ([audit log](#audit-log)) |

Pictures are PNG, JPEG or WebP images, cropped to a square.

### Single sign-on

Under **Security**, **Connect an identity provider** signs members in through your OpenID Connect provider. Create an app for Kaeso at the provider first, then answer five steps:

1. **Where is your identity provider?** The app's issuer URL, such as `https://login.example.com`.
2. **Which app signs people in?** Its client id and client secret.
3. **Which email domains sign in with it?** People with an address at these domains are sent to the provider. Internationalised domains work in either form, `bücher.example` or `xn--bcher-kva.example`.
4. **Can people join without an invitation?** **Invite only**, or the role people new to Kaeso join with. It can't be Owner.
5. **How does the sign-in page offer it?** The name on its button, such as `Acme ID` for **Continue with Acme ID**; left empty, the button says **Continue with SSO**. **Start without a click** sends people straight to the provider whenever the sign-in page offers it alone, except right after they signed out or the provider refused them.

Once connected, the page shows:

- the **Redirect URL**, to register at the provider as the sign-in callback;
- the **Sign-in link**, which opens the sign-in page with this provider's button and no address to type; share it with members, for example on your intranet.

**Edit single sign-on** changes the connection; an empty secret keeps the stored one. **Disconnect** removes it, and members sign in with a password or a sign-in link again.

An existing Kaeso account signs in through single sign-on only in organizations it already belongs to: invite it, and it accepts the invitation by signing in another way. [Sign-in and single sign-on](../self-hosting/sign-in.md) has every rule, and how to run an install on single sign-on alone.

### Requirements

Also under **Security**:

| Setting | What it does |
| --- | --- |
| Require single sign-on | Members get into the organization only through its identity provider; other sessions keep working for their other organizations. Needs a connected provider |
| Require two-factor authentication | Members who sign in with a password need two-factor authentication. Sign-in links and single sign-on are not affected |
| Maximum key lifetime | The longest a new key may live, in days, or **No limit**. Existing keys keep their expiry, and projects can set a shorter limit |

If you didn't sign in through the provider, or don't use two-factor authentication yet, turning the requirement on closes the organization for you too until you do; the console says so before it saves.

### Deleting an organization

If the install allows it, someone with the right **Delete organization**, by default an owner, chooses **Delete organization** under **General** and types the organization's name to confirm.

- Every API key of the organization stops working at once, and members are told by email.
- For the grace period, 7 days unless the install sets another, the organization stays read-only, every page shows a banner, and an owner can **Restore organization**.
- Then its projects, keys, provider keys, settings, usage and logs are deleted for good.

### Audit log

The organization's **Audit log**, in its settings, records every change in the organization; a project's **Audit log**, in the project's sidebar, every change in that project. Each entry shows when it happened, who made it, the action and its target; filter by actor, action, target id and days. Choose an entry to see the IP address, user agent and request id, any change of rights, and the values before and after.

| Action | Right |
| --- | --- |
| Read the organization's audit log | **View audit log** |
| **Export** it as JSON Lines, with the current filters | **Export audit log** |
| Read a project's audit log | **View project audit log** |

## Projects

**Projects** lists the organization's projects; archived ones are under **All**. **Create project** asks for a name and makes you the project's admin; it needs the right **Create projects**.

The **default project** is the one every member joins and new members start in. **Make default** in another project's menu moves it there; the previous default stays a project, with its members. This needs the right **Edit organization**.

### Overview

A project's **Overview** shows requests, tokens, cost and errors for a period, a breakdown by time, model, key or member, the project's limits with what is used, and recent errors.

### Settings

| Setting | What it does |
| --- | --- |
| Picture, project name | Shown in the sidebar and in request logs |
| Project id | The id the Platform API uses |
| Maximum key lifetime | **Use the organization's limit**, or a shorter maximum for new keys of this project |
| Content logging | Whether requests and replies are kept with their log entries; it can't be turned on while the organization has it off |
| Archive project | Archives the project with its keys and service accounts, permanently; its keys stop working at once |

Projects are archived, never deleted, so their usage and logs stay. The default project can't be archived; make another project the default first.
