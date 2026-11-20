---
title: Share usage
description: Share read-only links to usage, a member's stats or a project's budgets with people who have no account.
order: 10
---

A shared link shows the numbers of one page to people without a Kaeso account, such as finance or a manager. It stays current, or keeps the numbers of the day it was made, and stops working when it expires or you revoke it.

## What you can share

The **Share** button is where each view is shown:

| Shared | Where | What the link shows |
| --- | --- | --- |
| Analytics | **Analytics** of the organization or a project | The totals of the period, the chart of the metric you choose against the days before, and the usage by project, model, key, member, route and client |
| Member stats | A member's sheet under **Organization settings** → **Members** | One member's requests, tokens, cost, cache hits and trend, by model and project |
| Budget status | A project's **Limits** | The project's limits and budgets with what they used and where they are heading |
| Project summary | A project's **Overview** | The project's last 30 days: requests, cost, error rate and top models |

Request logs are never shared: they can hold prompts and replies. Nor are keys, members' addresses or ids.

## Choose what a link shows

The dialog of **Share** sets:

- **Title**, which the page shows. It names the project, organization or member unless you hide that name.
- **Numbers.** **Live** shows the last days whenever the link is opened, as many as the page's period. **Snapshot** keeps the period's numbers as they are now.
- **Show costs**, off by default. It's offered only when you can see the costs.
- **Hide** member names, key names and project names. Hidden names show as *Member 1*, *Key 2* and so on, by the number of requests.
- **Who** can open it: **Members of this organization, signed in**, or **Anyone with the link**, where the organization allows it.
- **Expires** after 1, 7, 30 or 90 days, or never, within the organization's longest expiry.

The link is shown once. Kaeso keeps only its hash, so copy it before you close the dialog.

A link never shows more than its maker can see. Each time it is opened, Kaeso checks that the maker is still a member with the right to share and to see what the link shows. A link whose maker left, or lost those rights, stops working; one whose maker lost the right to see costs keeps working without them.

## The shared page

The page opens at `https://kaeso.example.com/share/…` in the viewer's language and light or dark mode, on a phone or a computer. It has no links into the console, and search engines are asked not to index it. Chats and posts show a preview image of the chart. A small **Made with Kaeso** mark sits at its foot, which an instance admin can turn off.

## Manage links

**Organization settings** → **Shared links** lists the links that still work, with their audience, maker, views, last opening and expiry. **Revoke** stops a link at once. Without `usage.share.manage` you see and revoke only your own links.

Under **Organization settings** → **Security**, **Shared links** sets **Allow shared links** (**Off**, **Members only** or **Anyone with the link**) and the **Longest expiry**. Tightening the policy applies to links made before: with **Off** they stop working, and with **Members only** they need a member's sign-in. Instance admins can turn shared links off for the whole install ([instance administration](instance-administration.md#change-install-settings)).

Making, opening and revoking a link are in the audit log. Holders of `usage.share.manage` get an email when someone shares a link, and the organization's webhooks for **Security settings changed** get a `share.created` event ([webhooks](webhooks.md)).

## Rights

| Right | Allows |
| --- | --- |
| `usage.share` | Share what you can see and revoke your own links. Owners, admins, billing, developers and members hold it |
| `usage.share.manage` | See and revoke every link, and set the policy. Owners and admins hold it |

Sharing an organization's Analytics needs `usage.org.view`, a project's Analytics or summary `usage.view` in it, and its budgets `limits.view`. Sharing someone else's stats needs `usage.org.view`; your own need nothing more. Showing costs needs `costs.org.view` or `costs.view`.

On the Platform API, `POST /platform/v1/orgs/{org}/shared-links` makes a link, `GET` lists them and `DELETE …/shared-links/{link}` revokes one ([Platform API](../api/platform-api.md)). Agents connected to Kaeso's MCP server get the same as tools ([MCP server](../api/mcp.md)).

## Next steps

- [Organizations and projects](organizations-and-projects.md): Read what Analytics shows before you share it.
- [Members and roles](members-and-roles.md): Give or take the rights to share.
- [Webhooks](webhooks.md): Send new shared links to a chat or an on-call tool.
