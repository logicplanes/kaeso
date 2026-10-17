---
title: Using the console
description: Find your way around the console, the web interface of a Kaeso install.
order: 3
---

The console runs at your install's domain, such as `https://kaeso.example.com`. You sign in with your email address and, depending on what the install and your organization allow, a password, a sign-in link sent by email, or single sign-on; [sign-in and single sign-on](../self-hosting/sign-in.md) explains what the sign-in page offers.

## The sidebar

The sidebar follows where you work:

- **In an organization:** **Overview**, **Projects** and **Providers**, then the organization's projects.
- **In a project:** **Overview**, **Playground**, **API keys**, **Request logs**, **Models**, **Limits**, **Members**, **Service accounts**, **Audit log** and **Settings**, under a link back to the organization.

The menu at the foot of the sidebar switches organization and opens **Organization settings**, your **Account** and, for instance admins, **Instance**. **Create organization** and **Sign out** are there too.

## Rights

What you see depends on your rights. A page you have no right to is left out of the sidebar. An action you can't take is disabled, and its tooltip names the right it needs and who can give it to you. [Members and roles](members-and-roles.md) explains roles and rights.

Some actions need a sign-in from the last 15 minutes; the console asks you to sign in again before it carries them out:

- in an organization: adding or changing provider keys, giving a key platform access, creating platform keys, changing or requiring single sign-on, deleting logs, transferring ownership and deleting the organization;
- in your account: changing your email address, setting a password when you have none, turning off two-factor authentication, regenerating recovery codes and deleting the account.

## Search and shortcuts

⌘K, or Ctrl+K, opens search: go to any page; find the current project's keys, models, members and service accounts; create things; switch organization or project; or change the theme.

Single keys work when no field has focus:

| Keys | Action |
| --- | --- |
| `c` | Create on this page, such as a key on **API keys** |
| `/` | Search this page |
| `[` | Collapse or expand the sidebar |
| `g` `o`, `g` `k`, `g` `l`, `g` `m`, `g` `p` | Go to the project's overview, API keys, request logs, models or settings |
| `g` `s`, `g` `a` | Go to the settings or the organization's audit log |
| `?` | List every shortcut |

A list, such as API keys or members, is a single Tab stop. Inside it, the up and down arrows or `j` and `k` move between rows and Enter opens one; the right arrow moves into a row's actions and the left arrow or Escape back out. Where rows can be selected, as in **Add models**, Space or `x` selects a row, Shift with an arrow extends the selection, ⌘A or Ctrl+A selects every row and Escape clears it. The first Tab on a page reaches **Skip to content**, which jumps past the sidebar.

## Pages

| Page | What it covers |
| --- | --- |
| [Organizations and projects](organizations-and-projects.md) | Creating and switching them, overviews, settings, single sign-on, the audit log |
| [Members and roles](members-and-roles.md) | Invitations, roles and rights, service accounts, model access |
| [Provider keys](provider-keys.md) | Connecting OpenAI and Anthropic |
| [Models and aliases](models-and-aliases.md) | The project's catalogue, availability, overrides and aliases |
| [API keys](api-keys.md) | Keys for apps and scripts, and platform keys |
| [Limits and budgets](limits-and-budgets.md) | Rates and budgets for projects, members and keys |
| [Request logs](request-logs.md) | Every request, its content, exports and retention |
| [Playground](playground.md) | Trying models side by side |
| [Account and language](account.md) | Your profile, security, sessions, theme and language |
| [Instance administration](instance-administration.md) | The users, organizations and settings of the install |
