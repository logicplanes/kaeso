---
title: Model lifecycle
description: How Kaeso learns that a provider retires a model, warns the projects that still call it, and moves them to the replacement.
order: 5
---

Providers retire models on fixed dates, and a request to a model after its shutdown day fails. Kaeso tracks each retirement from the provider's announcement to the shutdown: [KPMR](../kpmr/index.md) publishes the dates and the replacement the provider recommends, and the gateway works out which projects still call the model, warns them ahead of time and moves them with one click, or on its own where a project allows it.

## What KPMR publishes

Every model in the catalogue has a `lifecycle`: the days the provider announced the retirement, deprecates the model and shuts it down, the `replacement` it recommends and the `source` of the announcement, each `null` until announced ([catalogue format](../kpmr/catalogue.md#lifecycle)). The model's `status` becomes `deprecated` and then `retired` on those days, and a retired model stays in the catalogue. KPMR reviews every date before it publishes it ([retirements and model lines](../kpmr/lifecycle-and-lines.md)).

## Where a retiring model shows

- **Models page:** a model with a shutdown date carries **Shuts down** and the date, and its tooltip names the replacement. A banner appears when a model of the project shuts down within 90 days.
- **Retiring tab:** every model of the project that its provider retires, soonest shutdown first. Each opens a sheet with the dates, the replacement and how its prices differ, the aliases and route steps that call the model (fallbacks and split branches included), and who called it in the last 30 days: keys, members, service accounts and client apps, by the user agent they send. Each caller links to its requests in the [request logs](../console/request-logs.md), and a chart shows the requests per day since the announcement, so you can see whether the move is happening.
- **Organization overview:** a banner lists the projects that use retiring models, with the first shutdown of each.

The callers and the chart need `usage.view`; the rest needs `models.view`.

## Replies carry the dates

A reply from a model with announced dates carries three standard headers, so SDKs, proxies and your own logging see the warning without anyone opening the console:

```http
Deprecation: @1793750400
Sunset: Thu, 04 Feb 2027 00:00:00 GMT
Link: <https://kpmr.kaeso.app/models/anthropic/claude-sonnet-5-5>; rel="successor-version", <https://example.com/deprecations>; rel="deprecation"; type="text/html"
```

`Deprecation` ([RFC 9745](https://www.rfc-editor.org/rfc/rfc9745)) is the deprecation day as Unix seconds, `Sunset` ([RFC 8594](https://www.rfc-editor.org/rfc/rfc8594)) the shutdown day, and `Link` points at the replacement's KPMR page and the provider's announcement.

## After the shutdown

A request that names a retired model, directly or through an alias step, fails with `410` and a Kaeso error that names the day and the replacement, instead of the provider's `404`:

```text
The model `anthropic/claude-sonnet-4-5-20250929` was retired on 2026-11-30; its provider recommends `claude-sonnet-5-5`.
```

In an alias, the step fails as unavailable, so the route [falls back](alias-routes.md#fallbacks) to its next step.

## Moving to the replacement

**Move to replacement** in a retiring model's sheet shows every route step that would change and the price difference, then changes them in one step: it adds the replacement to the catalogue when the project doesn't have it, with KPMR's prices, and points every alias step that called the model at it. The model itself stays in the catalogue, so apps that name it directly keep working until the shutdown. The move needs `models.manage` and is recorded in the audit log as `catalogue_model.move`.

Two project settings, under **Settings → Model retirement**, act on the shutdown day by themselves. Both are off by default and need `models.manage`; changing them is recorded as `project.model_lifecycle`:

| Setting | What it does |
| --- | --- |
| **Move aliases on shutdown** | On the shutdown day, Kaeso moves the aliases and route steps that call the model to its replacement, as **Move to replacement** does, and sends a notice and a `model.moved` webhook event |
| **Answer retired models with the replacement** | After the shutdown, a request that names the retired model is answered by the replacement instead of failing. `kaeso-model` names the model that answered, and the request log shows the retired model as a skipped step. Off by default because the price and the behaviour change |

The replacement answers only when the caller may use it. Kaeso adds it to the catalogue on the shutdown day when the project doesn't have it.

## Warnings

Two notices in the **Project** area of [your emails](../console/account.md#choose-your-emails) go to the members of a project who can see its models, and only for models in that project's catalogue:

| Notice | When |
| --- | --- |
| Model deprecated | KPMR publishes a shutdown date or a replacement for a model the project uses |
| Model shuts down soon | 30, 7 and 1 days before the shutdown, while the project still called the model in the last 7 days; and when Kaeso moved its aliases on the shutdown day |

Each comes as a [webhook event](../console/webhooks.md) too: `model.deprecated`, `model.shutdown_soon` and `model.moved`, with the catalogue model's id, the dates, the replacement and, for `model.shutdown_soon`, the requests of the last 7 days. The gateway checks every hour.

## In the Platform API and MCP

| Endpoint | What it does | Right |
| --- | --- | --- |
| `GET /orgs/{org}/projects/{project}/deprecations` | The retiring models with their aliases, route steps and callers | `models.view` |
| `GET /orgs/{org}/projects/{project}/deprecations/{model}/move` | What a move would change | `models.view` |
| `POST /orgs/{org}/projects/{project}/deprecations/{model}/move` | Moves to the replacement | `models.manage` |
| `GET`, `PUT /orgs/{org}/projects/{project}/model-lifecycle` | The two settings | `models.view`, `models.manage` |
| `GET /orgs/{org}/deprecations` | The organization's projects that use retiring models | `org.view` |

Kaeso's [MCP server](../api/mcp.md) offers the same actions as tools. The counter `kaeso_retired_model_requests_total{outcome}` counts steps that named a retired model, by whether they were refused or answered by the replacement ([monitoring](../self-hosting/monitoring.md)).

## Next steps

- [Retirements and model lines](../kpmr/lifecycle-and-lines.md): See where the dates come from and how KPMR reviews them.
- [Alias routes](alias-routes.md): Route requests across models with fallbacks.
- [Webhooks](../console/webhooks.md): Receive the warnings in your own systems.
