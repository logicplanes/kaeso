---
title: Members and roles
description: Invite people, give them roles and rights, and create service accounts for software.
order: 2
---

Everyone in an organization has an organization role, and a project role in each project they belong to. A role is a set of rights, and you can also add or remove single rights for a member. [Rights](../concepts/rights.md) lists every right and the roles that hold it.

What you see in the console depends on your rights. A page you have no right to is left out of the sidebar. An action you can't take is disabled, and its tooltip names the right it needs and who can give it to you.

## Invite people

Open **Organization settings** › **Members** and choose **Invite members**. Then answer three steps:

1. **Who are you inviting?** Type or paste one or more addresses. An address you typed counts as soon as you continue; one that isn't an email address is marked until you remove or fix it.
2. **What can they do in the organization?** Choose their organization role.
3. **Which projects?** Add the projects they join, each with a role. They always join the default project.

**Send invitation** emails each person a link that works for 21 days. **Invites** lists the pending invitations, where you can **Resend** or **Revoke** each one. Accepted, revoked and expired invitations are under **All**. Once someone accepts an invitation, their other pending invitations to the organization are revoked. An invitation never changes the roles of someone who is already a member. On **Members**, search by name or email address, or choose a role to see who holds it.

An organization with [single sign-on](organizations-and-projects.md#single-sign-on) can also let people from its email domains join without an invitation. That works only for people new to Kaeso. Someone who already has an account needs an invitation, and links the account to the identity provider from their **Account**.

In an organization with single sign-on, **Members** marks each member whose address isn't at one of its verified domains as **Guest**. The organization manages the other members' accounts, and **Require single sign-on** applies to those alone; guests keep their own way of signing in ([managed accounts](../self-hosting/sign-in.md#managed-accounts)).

## Choose a role

Organizations have six built-in roles: **Owner**, **Admin**, **Billing**, **Developer**, **Auditor** and **Member**. Projects have three: **Admin**, **Developer** and **Viewer**. The console describes each role wherever you choose one, and built-in roles can't be changed.

- Owners hold every right, including rights that later releases add. Only owners make, change or remove owners, and an organization always keeps at least one.
- Owners and admins hold every right in every project.
- In the default project, owners and admins start as Admin, developers as Developer, and everyone else as Viewer.

Two rules apply to every change you make. You can give only rights you hold yourself. And you can change only members who hold no right that you lack.

## Change a member's role or rights

Choose a member on **Members** to open their sheet:

- **Role** shows their role. **Customize** adds or removes single rights. A right that another of their rights needs is added and removed with it.
- **Access** lists every right they hold, and the role or right it comes from.
- Their projects list their role in each.
- **Transfer ownership**, for owners, makes the member an owner and you an admin.
- **Remove from organization** takes them out of every project and archives their keys in the organization. On your own sheet, this is **Leave organization**.

## Create a custom role

Open **Organization settings** › **Members** › **Roles** and choose **Create role**. A role is for the organization or for projects, and it starts from a built-in role or from no rights. An organization role can also give rights in every project. To start from a built-in role, open it and choose **Duplicate**.

- An organization has at most 50 custom roles.
- A change to a role applies at once to everyone who holds it.
- The console refuses a change that would leave a holder with a right but without a right it needs. The message names both rights and how many holders it affects.
- You can't delete a role somebody holds. Give them another role first.

## Add people to a project

On a project's **Members** page, choose **Add member** and pick someone from the organization with a project role. Invite people outside the organization first.

A project member's sheet also holds their [model access](#limit-the-models-someone-can-call) and [limits](limits-and-budgets.md). **Remove from project** archives their keys in the project, and they stay in the organization. Rights from a member's organization role count too: you can change or remove them only if you hold all their rights in the project. Only owners change or remove owners.

## Create a service account

A service account is a project member for software, such as a deployment, a job or a pipeline. Its keys keep working when the people who created them leave.

1. On a project's **Service accounts** page, choose **Create service account**.
2. Say what uses it, such as `checkout-production`.
3. Choose its project role. Its keys act with that role's rights.

Its sheet holds its keys, with **Create key**, its model access and its limits. **Archive service account** archives it and its keys for good.

Creating a service account needs the right **Create service accounts**, and creating its keys **Create service account keys**.

## Limit the models someone can call

Members and service accounts may call every model in the project's catalogue, and a key every model its owner may call. To narrow this, open **Models** in a member's, service account's or key's sheet and choose specific models.

A key never calls a model its owner may not. When you narrow a member's models, their keys' lists narrow to match. Changing model access needs the right **Manage limits**.

## Next steps

- [API keys](api-keys.md): Create keys for members and service accounts.
- [Limits and budgets](limits-and-budgets.md): Cap what a member, a service account or a key may spend.
- [Rights](../concepts/rights.md): Look up every right and the roles that hold it.
