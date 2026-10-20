---
title: Limits and budgets
description: Cap the requests, tokens and spending of a project, a member or a key.
order: 6
---

Limits cap the requests, tokens and spending of a project, of a member or service account, or of a key. A request must pass every limit that applies to it: its key's, its owner's and its project's.

## Add a limit

1. On a project's **Limits** page, choose **Add limit**.
2. **Who does it apply to?** Choose the project, a member or service account, or a key.
3. **What's the limit?** Choose its kind, amount, period and model. For a budget, also choose whether it's hard and when it alerts.

The page lists every limit with what has been used. **All**, **Project**, **Members** and **Keys** show one level at a time. A member's, service account's or key's limits are also in their sheet.

Seeing limits needs the right **View limits**, and changing them **Manage limits**.

## Kinds of limits

| Kind | Counts | Period |
| --- | --- | --- |
| Spend budget | US dollars, at the model's prices | A day, a month or in total |
| Token budget | Tokens | A day, a month or in total |
| Request rate | Requests | A sliding minute |
| Token rate | Tokens | A sliding minute |

Days and months are calendar days and months in UTC, and a total never resets. A limit applies to every model or to one.

A limit can't be higher than the same limit one level up. A key's spend budget stays within its owner's, and a member's within the project's.

## Hard and soft budgets

A rate always refuses requests above it, and never alerts. A budget is hard while **Refuse requests when used up** is on, as it is for a new limit. A hard budget refuses requests until its period resets. A soft budget lets requests through and only alerts.

A budget alerts at up to 10 percentages of its amount, by default at 50, 80 and 100 percent. At each, Kaeso emails the organization's owners and admins and the project's admins. For a key's budget, it also emails the key's owner. Anyone can turn these emails off under **Budget alerts** in their account.

## Change a limit

Editing a limit changes its amount and, for a budget, its alerts and whether it's hard. To change its kind, period or model, remove it and add another.

If someone else changed the limit after the page loaded, saving says so and loads the current limit. Make your change again on top of it.

## What an app sees

Kaeso answers a refused request with status 429, in the app's own API format, with a message that names the limit:

- Over a rate, the message says when to retry, and the `Retry-After` header gives the seconds.
- With a budget used up, the message says when the budget resets.

While requests still running could use up a hard budget or a token rate, Kaeso refuses the next request with `Retry-After: 1` until they finish. Each running request holds what it can cost at most: its input, estimated from its size, and its output limit (`max_tokens` or `max_output_tokens`, or else the model's own) at the model's prices. SpaceX's limit leaves reasoning out, so a request to a SpaceX reasoning model holds the model's own output limit. Under a budget, it holds at least what an average request cost in the period, or in the 30 days before. A stream holds it for as long as it runs.

A request with nothing else running always passes, so requests side by side never overrun a limit by more than one request. To run more requests side by side under a small budget, send a lower output limit. A request to a model without an output limit runs alone under a hard budget with no finished request in its period or the 30 days before.

> [!NOTE]
> Kaeso counts limits in Valkey. While Valkey can't be reached, rates and soft budgets let requests through, and hard budgets refuse them with status 503.

## Next steps

- [API keys](api-keys.md): Give each app its own key, so its limits are its own.
- [Request logs](request-logs.md): Find the requests a limit refused.
- [Errors](../api/errors.md): See how a refused request looks in each API format.
