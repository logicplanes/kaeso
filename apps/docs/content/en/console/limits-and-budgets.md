---
title: Limits and budgets
description: Cap request and token rates and spending for projects, members and keys.
order: 6
---

Limits keep usage inside bounds you set. They apply at three levels, and a request must pass all of them: its key's, its member's and its project's.

## Kinds

| Kind | Counts | Period |
| --- | --- | --- |
| Spend budget | US dollars, at the model's prices | A day, a month or in total |
| Token budget | Tokens | A day, a month or in total |
| Request rate | Requests | Per minute, sliding |
| Token rate | Tokens | Per minute, sliding |

Days and months are calendar days and months in UTC. A total never resets.

A limit applies to every model or to one model. A limit can't be higher than the same limit on the level above: a key's spend budget stays within its member's, and a member's within the project's. Amounts are positive. Requests and tokens are whole numbers up to 10^12, a million million; a spend budget is at most 1,000,000,000 US dollars, with up to nine decimals.

## Hard and soft

Rates always refuse requests over them. A budget is **hard** when **Refuse requests when used up** is on, as it is for a new limit: requests are refused until the period resets. A **soft** budget lets requests through and only alerts.

A budget can alert at up to 10 percentages of its amount, by default at 50, 80 and 100 percent. Rates have no alerts, and a rate limit sent with alert thresholds is refused. The organization's owners and admins, the project's admins and, for a key's budget, the key's owner get an email at each, unless they turned budget alerts off in their account.

## Add a limit

On a project's **Limits** page, choose **Add limit**:

1. **Who does it apply to?** The project, a member or service account, or a key.
2. **What's the limit?** The kind, the amount, the period, the model and, for a budget, whether it's hard and its alerts.

The page lists every limit with what has been used, and **All**, **Project**, **Members** and **Keys** show one level at a time. Limits of a member, a service account or a key also appear in their sheets. Editing a limit changes its amount and, for a budget, its alerts and whether it's hard; to change its kind, period or model, remove it and add another. If someone else changed the limit after the page loaded it, saving says so and loads the current limit, so you can make the change again. Seeing limits needs the right **View limits**, and changing them **Manage limits**.

## What an app sees

A refused request gets status 429 in the app's own format, with a message that names the limit:

- Over a rate: the message says when to retry, and `Retry-After` gives the seconds.
- A budget used up: the message says when it resets.

When requests still in progress have nearly used up a budget, the next request is refused with `Retry-After: 1` until they finish. A hard budget with no finished request in its period or the 30 days before it runs one request at a time until the first one finishes, because what a request costs isn't known yet.

> [!NOTE]
> Limits are counted in Valkey. If Valkey can't be reached, rates and soft budgets let requests through and hard budgets refuse them with 503 until it's back.
