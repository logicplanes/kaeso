---
title: Using the console
description: Find your way around the console, the web interface of a Kaeso install.
order: 3
---

The console is served at your install's domain, such as `https://kaeso.example.com`. Sign in with your email: depending on what the install and your organization allow, with a password, a sign-in link by email, or single sign-on.

## Two modes

**The app** is where you work in an organization and its projects. The sidebar shows the organization's **Overview**, **Projects** and **Providers**, followed by a list of its projects, and in a project its **Overview**, **Playground**, **API keys**, **Request logs**, **Models**, **Limits**, **Members**, **Service accounts**, **Audit log** and **Settings**.

**Settings** holds your account, your organization's settings and, for instance admins, the install's settings. Open it from the menu at the foot of the sidebar with **Organization settings**, **Account** or **Instance**; the same menu switches organizations and has **Sign out**.

## What you see

What you see depends on your rights. A page you have no right to is not in the sidebar. An action you can't take is disabled, and its tooltip names the right it needs and who can give it to you. [Members and roles](members-and-roles.md) explains rights.

Some sensitive actions, such as adding a provider key or giving a key platform access, ask you to sign in again when your last sign-in is more than 15 minutes old.

## Search and shortcuts

Press ⌘K, or Ctrl+K, to open search: go to any page, find the current project's keys, models, members and service accounts, create things, switch organization or project, and change the theme. `c` creates on the current page, `g` followed by a letter goes to a page, such as `g k` for API keys, and `?` lists every keyboard shortcut.

## Pages

| Page | What it covers |
| --- | --- |
| [Organizations and projects](organizations-and-projects.md) | Creating and switching them, the default project, overviews and settings |
| [Members and roles](members-and-roles.md) | Inviting people, roles and rights, service accounts |
| [Provider keys](provider-keys.md) | Connecting OpenAI and Anthropic |
| [Models and aliases](models-and-aliases.md) | The project's catalogue, overrides and aliases |
| [API keys](api-keys.md) | Keys for apps and scripts |
| [Limits and budgets](limits-and-budgets.md) | Rates and budgets for projects, members and keys |
| [Request logs](request-logs.md) | Every request, its content, exports and retention |
| [Playground](playground.md) | Trying models side by side |
| [Account and language](account.md) | Your profile, security, sessions, theme and language |
| [Instance administration](instance-administration.md) | Users, organizations and settings of the install |
