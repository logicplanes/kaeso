---
title: Members and roles
description: Invite people, give them roles and rights, and create service accounts.
order: 2
---

Everyone in an organization has an **organization role**, and in each project they belong to, a **project role**. A role is a set of rights, and a member can also have single rights added or removed. [Rights](../concepts/rights.md) lists every right and what each role holds.

## Roles

Organizations have six built-in roles, **Owner**, **Admin**, **Billing**, **Developer**, **Auditor** and **Member**, and projects three, **Admin**, **Developer** and **Viewer**. The console describes each one wherever you choose a role. Built-in roles can't be changed.

- Owners hold every right, including those of later releases. Only owners make, change or remove owners, and an organization keeps at least one.
- Owners and admins hold every right in every project.
- In the default project, owners and admins start as Admin, developers as Developer and everyone else as Viewer.

Two rules apply to every change: you can give only rights you hold yourself, and you can change only members who hold no right you lack.

### Custom roles

Under **Organization settings** › **Members** › **Roles**, **Create role** makes a role for the organization or for projects, starting from a built-in role or from no rights. An organization role can also give rights in every project. To start from a built-in role, open it and choose **Duplicate**.

- An organization has at most 50 custom roles.
- A change applies at once to everyone who holds the role. It isn't saved if it would leave a holder with a right but without a right that one needs; the message names both and how many holders it affects.
- A role somebody holds can't be deleted; give them another role first.

## Inviting people

On **Organization settings** › **Members**, **Invite member** opens three steps:

1. **Who are you inviting?** Type an address and press Enter, or paste several.
2. **What can they do in the organization?** Their organization role.
3. **Which projects?** Projects they join, each with a role. They always join the default project.

**Send invitation** emails each person a link that works for 21 days. **Invites** lists pending invitations, to **Resend** or **Revoke**; accepted, revoked and expired ones are under **All**.

An organization with [single sign-on](organizations-and-projects.md#single-sign-on) can also let people from its email domains join without an invitation. That works only for people new to Kaeso; someone who already has an account needs an invitation.

## A member's sheet

Choose a member to open their sheet:

- **Role**, with **Customize** to add or remove single rights. A right that another of their rights needs is added and removed together with it.
- **Access**: every right they hold, and the role or right it comes from.
- Their projects, with their role in each.
- **Transfer ownership**, for owners: the member becomes an owner, and you an admin.
- **Remove from organization**: they leave every project, and their keys in the organization are archived. On your own sheet this is **Leave organization**.

## Project members

On a project's **Members** page, **Add member** adds someone who is already in the organization, with a project role; invite others to the organization first. A project member's sheet also holds their [model access](#model-access) and [limits](limits-and-budgets.md). **Remove from project** archives their keys in the project; they stay in the organization.

## Service accounts

A **service account** is a project member for software, such as a deployment, a job or a pipeline. Its keys keep working whoever leaves the organization.

On a project's **Service accounts** page, **Create service account** asks what uses it, such as `checkout-production`, and its project role, which its keys act with. Its sheet holds its keys, with **Create key**, its model access and its limits. **Archive service account** archives it and its keys, permanently.

Creating one needs the right **Create service accounts**, and creating its keys **Create service account keys**.

## Model access

Members and service accounts may call every model in the project's catalogue, and keys every model their owner may. Under **Models** in a member's, service account's or key's sheet, choose specific models instead. A key never calls a model its owner may not: narrowing a member's models narrows their keys' lists to match. Changing model access needs the right **Manage limits**.
