---
title: Webhooks
description: Send the organization's notices, signed, to on-call tools, chat and scripts.
order: 9
---

A webhook sends the organization's operational notices, such as a budget alert or a refused provider key, to an address of yours as they happen. On-call tools can open an incident from it, a chat channel can show it, and a script can act on it. Webhooks get their notices whatever anyone chose for their emails, and also on an install that sends no email.

**Webhooks** is a page of **Organization settings**. You need `webhooks.view` to see it and `webhooks.manage` to change it ([rights](../concepts/rights.md)). An organization has at most 10 webhooks.

## Add a webhook

1. Open **Organization settings** › **Webhooks** and choose **Add webhook**.
2. Name it, and enter the address that receives the events, such as `https://hooks.example.com/kaeso`.
3. Choose the format. **JSON** sends Kaeso's event, described below, for scripts and on-call tools. **Slack** sends one line of text, `{"text": "…"}`, which Slack's incoming webhooks and chat tools that read the same take.
4. Choose the notices it receives, and whether it gets the whole organization's or one project's.
5. Choose **Add webhook**, and copy the signing secret. It is shown only once; your receiver checks every delivery with it.

The address must be a public `https` address. An install can allow other networks, over `http` or `https`, with `KAESO_WEBHOOKS_ALLOWED_NETWORKS` ([configuration](../self-hosting/configuration.md#webhooks)). Kaeso checks every connection after DNS resolution, sends without a proxy and follows no redirects.

A webhook's menu has **Send test**, **Turn off** or **Turn on**, **Rotate secret** and **Delete**. **Send test** sends a `webhook.test` event at once, tried only once, and says whether it arrived. **Rotate secret** replaces the signing secret at once, so the receiver needs the new one before the next delivery. Opening a webhook shows what it receives and its deliveries of the last 7 days, the newest 50, each with its tries and the receiver's last answer.

## What a webhook can receive

| Notice | Event types |
| --- | --- |
| Budget alerts | `budget.alert` |
| Limit reached | `limit.reached` |
| Your key expires | `key.expiring` |
| Content logging changed | `project.content_logging_changed` |
| MCP servers need attention | `mcp_server.signed_out`, `mcp_server.tool_held` |
| Security settings changed | `organization.security_changed` |
| Member changes | `organization.member_joined`, `organization.member_left` |
| Provider keys need attention | `provider_key.refused`, `provider_key.signed_out`, `provider_key.spend_limit_reached`, `provider_key.unlinked` |
| Model changes | `model.delisted`, `model.price_changed` |

These are the notices about the organization and its projects ([emails](account.md#choose-your-emails)). A person's own notices, such as their account's security or an export they started, never go to a webhook. A webhook with a project gets that project's events only; one without gets every event of the organization and its projects. Key expiry is told for every key that expires, not only to its owner.

## The event

A JSON delivery is a `POST` with `content-type: application/json` and a body like this:

```json
{
  "id": "msg_01kb2c9w6t3m5q7r8s0v1x2y4z",
  "type": "budget.alert",
  "created_at": "2026-11-16T09:30:00Z",
  "organization": { "id": "org_01kb2c8a4b5c6d7e8f9g0h1j2k", "name": "Acme" },
  "project": { "id": "proj_01kb2c8m3n4p5q6r7s8t9v0w1x", "name": "Checkout" },
  "summary": "Checkout used 80% of its monthly spend budget",
  "data": {
    "limit_id": "lim_01kb2c8y2z3a4b5c6d7e8f9g0h",
    "kind": "spend_budget",
    "period": "month",
    "amount": "100",
    "used": "80.42",
    "percent": 80,
    "applies_to": { "type": "project" },
    "resets_at": "2026-12-01T00:00:00Z"
  },
  "url": "https://kaeso.example.com/orgs/org_01kb2c8a4b5c6d7e8f9g0h1j2k/projects/proj_01kb2c8m3n4p5q6r7s8t9v0w1x/limits"
}
```

- `id` is the event's id, the same in every try.
- `project` is `null` for an event of the organization.
- `summary` is the notice's line in English.
- `url` opens the console page the event is about.
- Ids have the form the [Platform API](../api/platform-api.md) uses. Amounts are strings: US dollars for spending, tokens or requests otherwise.

`data` holds:

| Type | `data` |
| --- | --- |
| `budget.alert` | `limit_id`, `kind` (`token_budget` or `spend_budget`), `period`, `amount`, `used`, `percent` (the threshold crossed), `applies_to` (`{"type": "project"}`, or `member` or `key` with its `name`), `resets_at` (`null` for a total) |
| `limit.reached` | `limit_id`, `kind`, `period`, `amount`, `applies_to`, `resets_at` |
| `key.expiring` | `key_id`, `key` (its name), `owner` (a user id, `null` for a service account's key), `expires_at`, `days` |
| `project.content_logging_changed` | `on`, `by` (who changed it) |
| `mcp_server.signed_out` | `server_id`, `server` (its name) |
| `mcp_server.tool_held` | `server_id`, `server`, `tool`: a tool that changed and waits for approval |
| `organization.security_changed` | `change` (`sign_in_rules`, `single_sign_on`, `agent_policy`, `key_policy`, `platform_key_created` or `platform_key_rotated`), `platform_key` (its name, for the last two), `by` |
| `organization.member_joined` | `person`, `sso` (whether they arrived through single sign-on) |
| `organization.member_left` | `person`, `removed_by` (`null` when they left) |
| `provider_key.refused` | `provider_key_id`, `provider`, `name`, `default`, `projects`, `refusal` with the provider's `status` and `message`, `billing` (a billing problem) and `first_at` and `last_at` |
| `provider_key.signed_out` | `provider_key_id`, `provider`, `name`, `default`, `projects`, `signed_out_at` |
| `provider_key.spend_limit_reached` | `provider_key_id`, `provider`, `name`, `default`, `projects`, `refused_at`, `resets_at` (when the provider lets requests through again at the latest) |
| `provider_key.unlinked` | `provider_key_id`, `provider`, `name`, `default`, `projects`, `unlinked_at` |
| `model.delisted` | `catalogue_model_id`, `provider_id`, `provider`, `model` |
| `model.price_changed` | `catalogue_model_id`, `provider_id`, `provider`, `model`, `prices`: each changed price (`input`, `output`, `cache_read`, `cache_write`, `cache_write_1h`) with its old and new US dollars per million tokens, `from` and `to`, `null` where it had or has no price |
| `webhook.test` | `webhook_id`, `name` |

`person`, `by` and `removed_by` are people's names; `by` and `removed_by` are `null` when no person made the change, such as a platform key. A model change comes once for each project whose catalogue has the model. New types and new fields in `data` may come in later releases; ignore those you don't know.

## Verify a delivery

Kaeso signs every delivery as [Standard Webhooks](https://www.standardwebhooks.com/) specifies, so its libraries can check them. Each request has three headers:

| Header | What it holds |
| --- | --- |
| `webhook-id` | The event's `id`. A receiver that sees an id twice has the same event again |
| `webhook-timestamp` | When this try was sent, in seconds since 1970 |
| `webhook-signature` | `v1,` and the base64 HMAC-SHA256 of `{webhook-id}.{webhook-timestamp}.{body}`, keyed with the secret's bytes after `whsec_`, base64-decoded |

Check the signature over the body exactly as received, before parsing it, and refuse a timestamp more than five minutes away from your clock:

```js title="verify.mjs"
import { createHmac, timingSafeEqual } from "node:crypto";

export function verify(secret, headers, body) {
  const id = headers["webhook-id"];
  const timestamp = headers["webhook-timestamp"];
  if (Math.abs(Date.now() / 1000 - Number(timestamp)) > 300) return false;
  const key = Buffer.from(secret.replace(/^whsec_/, ""), "base64");
  const expected = createHmac("sha256", key).update(`${id}.${timestamp}.${body}`).digest();
  return headers["webhook-signature"].split(" ").some((s) => {
    const [version, signature] = s.split(",");
    const given = Buffer.from(signature ?? "", "base64");
    return version === "v1" && given.length === expected.length && timingSafeEqual(given, expected);
  });
}
```

## Deliveries and tries

- **When.** An event goes out within seconds of its notice.
- **Delivered.** A `2xx` answer within 10 seconds delivers the event. Any other answer, a redirect included, or no answer is a failed try.
- **Tries.** A failed event is tried again after 1, 5, 15 and 60 minutes, five tries in all, each signed afresh. After the fifth it has failed for good.
- **At least once.** Events can come twice and out of order: tell them apart by `webhook-id`, and order them by `created_at`.
- **Turned off.** A webhook whose tries fail 20 times in a row is turned off: its pending events are dropped, the audit log records it, and the people who hold `webhooks.manage` get an email. **Turn on** sends it new events again and starts its count afresh.
- **Kept.** Deliveries are kept 7 days.

The gateway counts every try in `kaeso_webhook_deliveries_total{outcome}` ([monitoring](../self-hosting/monitoring.md#metrics)). Webhooks are part of the [Platform API](../api/platform-api.md) at `orgs/{org}/webhooks`, and adding, changing, rotating and deleting them is in the [audit log](organizations-and-projects.md#read-the-audit-log).

## Next steps

- [Limits and budgets](limits-and-budgets.md): Set the budgets whose alerts a webhook can carry.
- [Account and language](account.md): Choose which notices reach you by email.
