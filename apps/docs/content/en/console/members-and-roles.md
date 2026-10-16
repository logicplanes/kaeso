---
title: Members and roles
description: Invite people, give them roles and rights, and create service accounts.
order: 2
---

Everyone in an organization has an **organization role**, and in each project they belong to, a **project role**. A role is a set of rights; a member can also have single rights added or removed. The [rights reference](../concepts/rights.md) lists every right.

## Roles

| Organization role | What it allows |
| --- | --- |
| Owner | Every right, now and in future releases. Only owners manage owners, and an organization keeps at least one. |
| Admin | Manages the organization and every project, except deleting the organization, transferring ownership and changing billing |
| Billing | Billing, and usage and costs across all projects |
| Developer | Creates projects and sees usage, costs and provider keys; works in the projects they are added to |
| Auditor | Reads the whole organization for reviews, including the audit log and every project, without changing anything or seeing prompts and outputs |
| Member | Sees the organization and its members; works in the projects they are added to |

| Project role | What it allows |
| --- | --- |
| Admin | Manages the project: members, keys, service accounts, models, limits, logs and settings |
| Developer | Creates their own keys, calls models with them and in the Playground, and reads the project, including prompts and outputs |
| Viewer | Reads the project's settings, models, limits, keys, usage and log metadata, without changing anything |

Owners and admins hold every right in every project. In the default project, owners and admins join as Admin, developers as Developer, and everyone else as Viewer.

### Custom roles

Under **Organization settings** › **Members** › **Roles**, **Create role** makes a role of your own for the organization or for projects, starting from a built-in role or from no rights. An organization role can also hold rights in every project. Built-in roles can't be changed; **Duplicate** one to start from it. An organization has at most 50 custom roles, and a role somebody holds can't be deleted.

You can only give rights you hold yourself, and only change members whose rights you all hold.

## Inviting people

**Invite member** on the organization's **Members** page sends an invitation by email:

1. Enter one or more email addresses.
2. Choose their role in the organization.
3. Add the projects they join, each with a role. They always join the default project.

An invitation is valid for 21 days. **Invites** lists them, to **Resend** or **Revoke**. Organizations with single sign-on can also let people from their email domains join without an invitation.

## A member's sheet

Choose a member to open their sheet: their role, with **Customize** to add or remove single rights; every right they hold and where it comes from; their projects; and **Remove from organization**. An owner can **Transfer ownership** to them and become an admin.

Removing someone from an organization or a project archives their keys there.

## Project members

On a project's **Members** page, **Add member** adds someone who is already in the organization, with a project role; invite others to the organization first. A project member's sheet also sets which models they may call and their [limits](limits-and-budgets.md), and **Remove from project** takes them out while they stay in the organization.

## Service accounts

A **service account** is a member for software rather than a person: a backend, a job, a pipeline. Its keys don't depend on anyone staying in the organization.

On a project's **Service accounts** page, **Create service account** asks what uses it, such as `checkout-production`, and its project role. Its sheet holds its keys, with **Create key**, its model access and its limits. **Archive service account** archives it and its keys, permanently.

## Model access

Members and service accounts may call every model in the catalogue by default, and keys every model their owner may. In their sheet, under **Models**, choose specific models instead. A key can never call a model its member may not: narrowing a member's models narrows their keys' lists to match. Changing model access needs the right **Manage limits**.
