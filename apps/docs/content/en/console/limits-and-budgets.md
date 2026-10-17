---
title: Limits and budgets
description: Cap request and token rates and spending for projects, members and keys.
order: 6
---

Limits keep usage inside bounds you set, at three levels: the project, a member or service account, and a key. A request must pass every limit that applies to it, its key's, its member's and its project's.

## Kinds

| Kind | Counts | Period |
| --- | --- | --- |
| Spend budget | US dollars, at the model's prices | A day, a month or in total |
| Token budget | Tokens | A day, a month or in total |
| Request rate | Requests | A sliding minute |
| Token rate | Tokens | A sliding minute |

Days and months are calendar days and months in UTC, and a total never resets. A limit applies to every model or to one.

A limit can't be higher than the same limit on the level above: a key's spend budget stays within its member's, and a member's within the project's. Requests and tokens are whole numbers up to 10^12; a spend budget is at most 1,000,000,000 US dollars, with up to nine decimals.

## Hard and soft budgets

Rates always refuse requests above them. A budget is **hard** while **Refuse requests when used up** is on, as it is for a new limit: requests are refused until the period resets. A **soft** budget lets requests through and only alerts.

A budget alerts at up to 10 percentages of its amount, by default at 50, 80 and 100 percent. At each, an email goes to the organization's owners and admins, the project's admins and, for a key's budget, the key's owner, unless they turned off **Budget alerts** in their account. Rates have no alerts.

## Add a limit

On a project's **Limits** page, choose **Add limit**:

1. **Who does it apply to?** The project, a member or service account, or a key.
2. **What's the limit?** Its kind, amount, period and model and, for a budget, whether it is hard and when it alerts.

The page lists every limit with what has been used; **All**, **Project**, **Members** and **Keys** show one level at a time. A member's, service account's or key's limits are also in their sheet.

Editing a limit changes its amount and, for a budget, its alerts and whether it is hard; to change its kind, period or model, remove it and add another. If someone else changed the limit after the page loaded, saving says so and loads the current limit, so you can make your change again. Seeing limits needs the right **View limits**, changing them **Manage limits**.

## What an app sees

A refused request gets status 429 in the app's own API format, with a message that names the limit:

- over a rate, the message says when to retry, and `Retry-After` gives the seconds;
- with a budget used up, the message says when it resets.

When requests still running have nearly used up a budget, the next request is refused with `Retry-After: 1` until they finish. A hard budget without a finished request in its period or the 30 days before runs one request at a time until the first finishes, because what a request costs isn't known yet.

> [!NOTE]
> Limits are counted in Valkey. While Valkey can't be reached, rates and soft budgets let requests through, and hard budgets refuse them with status 503.
