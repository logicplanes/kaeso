---
title: Rights
description: Look up every right in an organization and a project, which roles hold it, and the rules for giving it.
order: 3
---

A right allows one kind of action and is written `resource.action`, such as `keys.all.view`. Every Platform API operation under `orgs/{org}` checks exactly one right, on the organization or the project in its path. [Members and roles](../console/members-and-roles.md) shows how to give roles and rights in the console.

Managing your own account needs no right. Instance administration isn't a right either: it is a flag on a user, and it gives no rights in organizations or projects.

## Notes

The tables below mark some rights with these notes:

- **Needs.** The right can be given only together with the rights it names. Every organization right also needs `org.view`, and every project right `project.view`. Kaeso refuses a change that gives a right without what it needs, but never changes rights already stored.
- **Sensitive.** The right exposes sensitive data or is hard to undo. Give it with care.
- **Recent sign-in.** Using the right from a console session needs a sign-in in the last 15 minutes.
- **Kaeso Cloud only.** The right has no effect on a self-hosted install.

## Roles

Every organization member has one organization role. Every project member and service account has one project role. You can also add or remove single rights on top of a role. A change to a role applies to everyone who holds it.

Built-in roles are the same in every organization, and nobody can change them. Their ids in the Platform API are their names in lowercase.

| Organization role | Description |
| --- | --- |
| Owner | Holds every right, now and in future releases; only owners manage owners. |
| Admin | Manages the organization and every project, except deleting it, transferring ownership and changing billing. |
| Billing | Manages billing and sees usage and costs across all projects. |
| Developer | Creates projects and sees usage, costs and provider keys; works in the projects they are added to. |
| Auditor | Reads the whole organization for reviews, including the audit log and every project, and changes nothing; never sees prompts or outputs. |
| Member | Sees the organization and its members; works in the projects they are added to. |

| Project role | Description |
| --- | --- |
| Admin | Manages the project: members, keys, service accounts, models, limits, logs and settings. |
| Developer | Calls models with their own keys and in the Playground, and reads the project, including prompts and outputs. |
| Viewer | Reads the project's settings, models, limits, keys, usage and log metadata; changes nothing. |

### What each built-in role holds

An organization role also holds rights in every project, on top of the member's own project role there:

| Organization role | Organization rights | Rights in every project | Starts in the default project as |
| --- | --- | --- | --- |
| Owner | All, including those of future releases | All | Admin |
| Admin | All except `org.delete`, `org.ownership.transfer` and `billing.manage` | All | Admin |
| Billing | `org.view`, `members.view`, `projects.view.all`, `logs.storage.view`, `usage.org.view`, `costs.org.view`, `billing.view`, `billing.manage` | None | Viewer |
| Developer | `org.view`, `members.view`, `projects.create`, `provider_credentials.view`, `logs.storage.view`, `usage.org.view`, `costs.org.view` | None | Developer |
| Auditor | `org.view`, `members.view`, `projects.view.all`, `provider_credentials.view`, `platform_keys.view`, `logs.storage.view`, `usage.org.view`, `costs.org.view`, `billing.view`, `audit_log.view`, `audit_log.export` | The Viewer's, and `project.audit_log.view` | Viewer |
| Member | `org.view`, `members.view` | None | Viewer |

| Project role | Rights |
| --- | --- |
| Admin | All project rights |
| Developer | The Viewer's, and `api.request`, `playground.use`, `keys.own.create`, `keys.own.manage`, `logs.content.view` |
| Viewer | `project.view`, `project.members.view`, `service_accounts.view`, `keys.all.view`, `models.view`, `limits.view`, `logs.metadata.view`, `usage.view`, `costs.view` |

Two organization rights also reach into every project. `projects.view.all` gives `project.view`, and `projects.archive.any` gives `project.view` and `project.archive`.

A member's rights in a project add up from three sources: their project role, their organization role's rights in every project, and what these two rights reach. In the default project, Owners and Admins start as project Admin, and Developers as Developer. Every other organization role starts as Viewer, custom roles included.

### Custom roles

- With `roles.manage`, an organization can define up to 50 custom roles, for the organization or for projects. Organization roles hold organization rights and rights in every project. Project roles hold project rights.
- Anyone with `org.view` sees the custom roles.
- A name has 1 to 50 characters and is unique per organization and scope, ignoring case. It can't be `custom` or the name of a built-in role of that scope. A description has at most 200 characters.
- Custom roles can't make owners. Owner is only the built-in role, and only owners hold `org.ownership.transfer`: no custom role or individual change can give it.
- You can't delete a role that a member, an active service account or a pending invitation holds. Nor can you delete the role that people who arrive through single sign-on get.

## Grant rules

Every change of roles and rights follows these rules:

1. **Give only what you hold.** You can give only rights you hold yourself, in the same scope: organization rights in the organization, project rights in that project. You can give rights in every project only if you hold them in every project. This applies to changing a member's role or rights, inviting and resending an invitation, adding a project member, creating or changing a service account or a custom role, and choosing the single sign-on role.
2. **Act only on peers.** You can change a member's role and rights, or remove the member, only if you hold every right the member holds in that scope. The same goes for changing or archiving a service account.
3. **Owners manage owners.** Only owners make, change, remove and invite owners, and only an owner transfers ownership. An organization never loses its last owner.
4. **Needs are checked.** Kaeso checks what every right a change gives needs. A change can't remove a right that a remaining right needs, even when that remaining right already lacks another of its needs. Kaeso refuses a change to a custom role that would leave a holder with a right but without one it needs. The holders' own rights never change with it.
5. **Keys have no rights of their own.** An API key acts as its member or service account in its project. An organization's platform key acts with the Admin role's rights in its organization.

[Platform API](../api/platform-api.md) describes the routes for rights, roles and grants.

## Organization rights

### Organization

| Right | Name | What it allows | Notes |
| --- | --- | --- | --- |
| `org.view` | View organization | See the organization, its settings and its roles. |  |
| `org.update` | Edit organization | Rename the organization, change its picture and general settings, and choose the default project. |  |
| `org.delete` | Delete organization | Schedule the organization for deletion, or restore it while it waits. | sensitive; recent sign-in |
| `org.ownership.transfer` | Transfer ownership | Make another member an owner in your place; only owners hold it. | sensitive; recent sign-in; needs `members.view` |

### Members & access

| Right | Name | What it allows | Notes |
| --- | --- | --- | --- |
| `members.view` | View members | See members, their roles and their rights. |  |
| `members.invite` | Invite members | Invite people by email with a role you could grant yourself. | needs `members.view` |
| `members.invites.revoke` | Cancel invites | Cancel pending invitations. | needs `members.view` |
| `members.remove` | Remove members | Remove members from the organization and archive their keys in it. | sensitive; needs `members.view` |
| `members.rights.update` | Change member roles | Change members' org roles and individual org rights. | needs `members.view` |
| `roles.manage` | Manage roles | Create, edit and delete the organization's custom roles. |  |

### Projects

| Right | Name | What it allows | Notes |
| --- | --- | --- | --- |
| `projects.view.all` | View all projects | See every project, including ones the member isn't in. |  |
| `projects.create` | Create projects | Create projects; the creator becomes their admin. |  |
| `projects.archive.any` | Archive any project | Archive any project and all its keys. | sensitive; needs `projects.view.all` |

### Providers

| Right | Name | What it allows | Notes |
| --- | --- | --- | --- |
| `provider_credentials.view` | View provider keys | See the organization's provider keys (metadata only, never secrets). |  |
| `provider_credentials.create` | Add provider keys | Add provider keys to the organization. | recent sign-in; needs `provider_credentials.view` |
| `provider_credentials.replace` | Change provider keys | Replace and rename provider keys, choose each provider's default and assign keys to projects. | recent sign-in; needs `provider_credentials.view` |
| `provider_credentials.delete` | Delete provider keys | Remove a provider key from the organization. | sensitive; needs `provider_credentials.view` |

### Security

| Right | Name | What it allows | Notes |
| --- | --- | --- | --- |
| `security.sso.manage` | Manage SSO | Configure the organization's identity provider and the role of people who arrive through it. | recent sign-in |
| `security.sso.require` | Require SSO | Require members to sign in to this organization through SSO. | sensitive; recent sign-in |
| `security.2fa.require` | Require two-factor | Require two-factor authentication for members. | recent sign-in |
| `security.key_policy.manage` | Set key lifetime | Set the organization's maximum key lifetime. |  |

### Keys

| Right | Name | What it allows | Notes |
| --- | --- | --- | --- |
| `platform_keys.view` | View platform keys | See the organization's platform keys (metadata). |  |
| `platform_keys.create` | Create platform keys | Create organization platform keys, which manage everything at org level. | sensitive; recent sign-in; needs `platform_keys.view` |
| `platform_keys.revoke` | Disable platform keys | Disable, enable or archive organization platform keys. | needs `platform_keys.view` |

### Logs & data

| Right | Name | What it allows | Notes |
| --- | --- | --- | --- |
| `logs.settings.manage` | Manage log settings | Set log retention and switch content logging on or off for the organization. |  |
| `logs.storage.view` | View log storage | See how many log entries exist and how much space they take. |  |
| `logs.delete` | Delete logs | Delete the organization's logs for a time range. | sensitive; recent sign-in; needs `logs.storage.view` |

### Usage & billing

| Right | Name | What it allows | Notes |
| --- | --- | --- | --- |
| `usage.org.view` | View org usage | See usage across all projects. |  |
| `costs.org.view` | View org costs | See costs across all projects. | needs `usage.org.view` |
| `billing.view` | View billing | See plan, invoices and payment status (Kaeso Cloud). | Kaeso Cloud only |
| `billing.manage` | Manage billing | Change the plan and payment method (Kaeso Cloud). | Kaeso Cloud only; needs `billing.view` |

### Audit

| Right | Name | What it allows | Notes |
| --- | --- | --- | --- |
| `audit_log.view` | View audit log | Read the organization's audit log. |  |
| `audit_log.export` | Export audit log | Export the organization's audit log. | needs `audit_log.view` |

## Project rights

### Project

| Right | Name | What it allows | Notes |
| --- | --- | --- | --- |
| `project.view` | View project | See the project and its settings. |  |
| `project.update` | Edit project | Rename the project and change its settings and picture. |  |
| `project.archive` | Archive project | Archive this project and all its keys. | sensitive |

### Members & access

| Right | Name | What it allows | Notes |
| --- | --- | --- | --- |
| `project.members.view` | View members | See the project's members, their roles and their rights. |  |
| `project.members.add` | Add members | Add organization members to the project with a role you could grant yourself. | needs `project.members.view` |
| `project.members.remove` | Remove members | Remove members from the project and archive their keys in it. | needs `project.members.view` |
| `project.members.rights.update` | Change member roles | Change members' project roles and individual project rights. | needs `project.members.view` |
| `service_accounts.view` | View service accounts | See the project's service accounts and their rights. |  |
| `service_accounts.create` | Create service accounts | Create service accounts with rights you hold yourself. | needs `service_accounts.view` |
| `service_accounts.rights.update` | Change service accounts | Rename service accounts and change their roles and rights. | needs `service_accounts.view` |
| `service_accounts.archive` | Archive service accounts | Archive service accounts and their keys. | needs `service_accounts.view` |

### Model use

| Right | Name | What it allows | Notes |
| --- | --- | --- | --- |
| `api.request` | Call models with keys | Call the project's models with one's own API keys. |  |
| `playground.use` | Use the Playground | Call the project's models from the console Playground. |  |

### Keys

| Right | Name | What it allows | Notes |
| --- | --- | --- | --- |
| `keys.own.create` | Create own keys | Create API keys for oneself. |  |
| `keys.own.manage` | Manage own keys | Disable, enable or archive one's own API keys. |  |
| `keys.all.view` | View all keys | See every key in the project (metadata). |  |
| `keys.all.manage` | Manage all keys | Disable, enable or archive anyone's keys in the project. | needs `keys.all.view` |
| `service_accounts.keys.create` | Create service account keys | Create keys for the project's service accounts. | needs `service_accounts.view` |
| `keys.platform_access.grant` | Give keys platform access | Give a new key access to the Platform API. | sensitive; recent sign-in |
| `security.key_policy.project.manage` | Set key lifetime | Set the project's maximum key lifetime. |  |

### Models

| Right | Name | What it allows | Notes |
| --- | --- | --- | --- |
| `models.view` | View models | See the project's models, aliases and routing. |  |
| `models.manage` | Manage models | Add, change and remove models and aliases. | needs `models.view` |

### Limits & budgets

| Right | Name | What it allows | Notes |
| --- | --- | --- | --- |
| `limits.view` | View limits | See project, member and key limits, budgets and model access. |  |
| `limits.manage` | Manage limits | Change project, member and key limits, budgets and model access. | needs `limits.view` |

### Logs & data

| Right | Name | What it allows | Notes |
| --- | --- | --- | --- |
| `logs.metadata.view` | View request logs | See request logs: model, tokens, cost, latency, status and key. |  |
| `logs.content.view` | View prompts and outputs | See request and response bodies in the logs. | sensitive; needs `logs.metadata.view` |
| `logs.export` | Export logs | Export the project's request logs. | sensitive; needs `logs.metadata.view` |
| `logs.content.manage` | Switch content logging | Switch content logging on or off for this project. | sensitive |

### Usage & costs

| Right | Name | What it allows | Notes |
| --- | --- | --- | --- |
| `usage.view` | View usage | See the project's usage. |  |
| `costs.view` | View costs | See the project's costs. | needs `usage.view` |

### Audit

| Right | Name | What it allows | Notes |
| --- | --- | --- | --- |
| `project.audit_log.view` | View project audit log | Read the audit log of changes in this project. |  |
