---
title: Organizations and projects
description: Create organizations and projects, connect single sign-on, and manage their settings.
order: 1
---

An organization holds members, provider keys and projects. A project holds a model catalogue, API keys, service accounts, limits and request logs. People join an organization with an organization role, and each of its projects with a project role.

## Create an organization

Open the menu at the foot of the sidebar and choose **Create organization**. You become its owner, and it starts with a default project. The install decides who may create organizations ([instance settings](instance-administration.md#change-install-settings)).

The same menu lists your organizations and switches between them.

## Read usage

The organization's **Overview** shows requests, tokens, cost and errors for a period you choose, from today to all time. It also breaks the cost down by project, and the usage by project, model or member. Seeing usage needs the right **View org usage**, and seeing cost **View org costs**.

A project's **Overview** shows the same numbers for one project, broken down by time, model, key or member. It also lists the project's limits with what is used, and its recent errors.

## Change organization settings

Open **Organization settings** in the menu at the foot of the sidebar. It has these pages:

| Page | What it holds |
| --- | --- |
| General | Picture, name and organization id, **Transfer ownership**, and deleting the organization |
| Members | Members, invitations and roles ([members and roles](members-and-roles.md)) |
| Security | Single sign-on, the sign-in requirements and the maximum key lifetime |
| Platform keys | Keys for scripts that manage the organization ([platform keys](api-keys.md#platform-keys)) |
| Logging | Content logging, retention, storage and log deletion ([request logs](request-logs.md#retention)) |
| Audit log | Every change in the organization ([audit log](#read-the-audit-log)) |

A picture is a PNG, JPEG or WebP image, and the console crops it to a square.

## Single sign-on

Single sign-on lets members sign in through your OpenID Connect identity provider. First create an app for Kaeso at your provider. Then, under **Security**, choose **Connect an identity provider** and answer five steps:

1. **Where is your identity provider?** The step shows the **Redirect URL** to register in the app as its sign-in callback. Enter the app's issuer URL, such as `https://login.example.com`.
2. **Which app signs people in?** Enter its client id and client secret.
3. **Which email domains sign in with it?** The sign-in page sends people with an address at these domains to your provider. Internationalized domains work in either form, `bücher.example` or `xn--bcher-kva.example`.
4. **Can people join without an invitation?** Choose **Invite only**, or the role that people new to Kaeso join with. That role can't be Owner.
5. **How does the sign-in page offer it?** Enter the **Name on the button**, such as `Acme ID` for "Continue with Acme ID". Left empty, the button says "Continue with SSO". **Start without a click** sends people straight to your provider when the sign-in page offers only that one, except right after they sign out or the provider refuses them. **Require verified addresses** refuses tokens that don't mark the address as verified; turn it off only for a provider that never sends `email_verified`.

Once you connect, the page shows two addresses:

- The **Redirect URL**: register it at your provider as the sign-in callback.
- The **Sign-in link**: it opens the sign-in page with your provider's button, so members don't type their address. Share it, for example on your intranet.

### Verify your email domains

Single sign-on signs in and admits only addresses at verified domains, so no organization can claim a domain it doesn't hold. For each domain, **Security** shows whether it is **Verified** or **Unverified**, and the TXT record an unverified one needs. Add it at your DNS provider, then choose **Verify** next to the domain:

```text
_kaeso-verify.acme.example  TXT  "kaeso-verify=Qm9ZtX3c8L1aV7pK2rN5wE0yH4uJ6sD9fG1bC3xT8eA"
```

DNS changes can take a while to show; choose **Verify** again later. Public mail domains, such as `gmail.com`, can't be added, and neither can a domain another organization verified. For a domain that public DNS doesn't answer for, an operator verifies it on the server with `kaeso sso verify-domain acme.example`.

**Edit single sign-on** changes the connection. An empty client secret keeps the stored one while the issuer and client id stay the same. **Disconnect** removes it, and members sign in with a password or a sign-in link again.

Single sign-on signs in an existing Kaeso account only once its owner links it to your provider, from their **Account** ([link single sign-on](account.md#link-single-sign-on)), and only if the account belongs to the organization. Invite the person, and they accept by signing in another way. Changing single sign-on needs a console session with a recent sign-in; platform keys can't. [Sign-in and single sign-on](../self-hosting/sign-in.md) has every rule, and how to run an install on single sign-on alone.

## Require single sign-on or two-factor authentication

Under **Security**, you can tighten how members get into the organization:

| Setting | What it does |
| --- | --- |
| Require single sign-on | Members get into the organization only through its identity provider. Their other sessions keep working for their other organizations. It needs a connected provider |
| Require two-factor authentication | Members need a sign-in with a code from their authenticator, with a password or a sign-in link. A sign-in through the organization's own identity provider is enough too |
| Maximum key lifetime | The longest a new key may live, in days, or **No limit**. Existing keys keep their expiry, and a project can set a shorter limit |

Turning on a requirement you don't meet yourself closes the organization to you too, until you sign in through the provider or set up two-factor authentication. The console warns you before it saves. Disconnecting the identity provider turns off **Require single sign-on** as well.

## Delete an organization

Under **General**, choose **Delete organization** and type the organization's name to confirm. You need the right **Delete organization**, which owners have, and the install must allow deleting organizations.

1. Every API key of the organization stops working at once, and Kaeso emails the members.
2. For a grace period, 7 days unless the install sets another, the organization is read-only and every page shows a banner. An owner can still choose **Restore organization**.
3. After the grace period, Kaeso deletes its projects, keys, provider keys, settings, usage and logs for good.

## Read the audit log

The organization's **Audit log**, in **Organization settings**, records every change in the organization. A project's **Audit log**, in the project's sidebar, records every change in that project.

Each entry shows when it happened, who made the change, the action and its target. Filter by actor, action, target id and days. Choose an entry to see the IP address, user agent and request id, any change of rights, and the values before and after.

| To | Right |
| --- | --- |
| Read the organization's audit log | **View audit log** |
| Export it as JSON Lines, with the current filters | **Export audit log** |
| Read a project's audit log | **View project audit log** |

## Create a project

On **Projects**, choose **Create project** and give it a name. You become the project's admin. Creating a project needs the right **Create projects**. Archived projects are listed under **Archived**, and every project under **All**. Search finds a project by name, and each project shows its requests in the last 30 days to people who can see the organization's usage.

The default project is the one every member joins, and where new members start. To move the default, choose **Make default** in another project's menu. The old default stays a project with its members. Moving it needs the right **Edit organization**.

## Change project settings

A project's **Settings** page holds these settings:

| Setting | What it does |
| --- | --- |
| Picture, project name | Shown in the sidebar and in request logs |
| Project id | The id the Platform API uses |
| Maximum key lifetime | **Use the organization's limit**, or a shorter maximum for new keys in this project |
| Content logging | Whether request and reply bodies are kept with their log entries. It can't be on while the organization has it off |
| Archive project | Archives the project with its keys and service accounts, for good. Its keys stop working at once |

Kaeso archives projects and never deletes them, so their usage and logs stay. The default project can't be archived: make another project the default first.

## Next steps

- [Members and roles](members-and-roles.md): Invite people and give them roles in the organization and its projects.
- [Provider keys](provider-keys.md): Connect the model provider keys your projects call models with.
- [Request logs](request-logs.md): Choose what the organization keeps, and for how long.
