---
name: kaeso
description: Use when the user asks about their Kaeso AI gateway, such as its organizations, projects, API keys, models, aliases and their routes, provider keys, limits, usage, costs, request logs or audit log, or wants to change any of them. Also use it to connect Claude to their Kaeso install.
---

# Working with Kaeso

Kaeso is a self-hosted AI gateway. The `kaeso` MCP server gives you one tool per Platform API operation you are allowed to use, named after its operation id, such as `get_orgs_org_usage`, and `show_usage_stats`, which draws usage as a view.

## Connecting Kaeso

If the `kaeso` MCP tools aren't available, connect the server first. You need the address of the user's Kaeso install, such as `https://kaeso.example.com`; ask for it if you don't have it.

- In Claude Code, the plugin asks for the server's address when it is installed, and `/plugin configure kaeso@kaeso` changes it. Otherwise run `claude mcp add --transport http --scope user kaeso https://<their install>/mcp`. Then tell them to run `/mcp`, choose `kaeso` and sign in through their browser, where they choose what you may do. If `/mcp` doesn't list `kaeso` yet, they restart Claude Code first.
- In Claude on the web, in the desktop and mobile apps and in Cowork, you can't run commands. Give them the link `https://claude.ai/customize/connectors?modal=add-custom-connector&connectorName=Kaeso&connectorUrl=` followed by their install's address and `/mcp`, percent-encoded, so `https://kaeso.example.com/mcp` becomes `https%3A%2F%2Fkaeso.example.com%2Fmcp`. It opens **Add custom connector** with the values filled in. Or they open **Customize** › **Connectors**, choose **Add custom connector** and enter the name `Kaeso` and that address. After **Connect**, they sign in through their browser; a new conversation then has the tools.

## Calling the tools

- Start with `get_me_memberships` to find the user's organizations. Ids look like `org_…` and `proj_…`.
- Every tool takes `agent_model`: the model family you are, such as `claude`, or `unknown` if none in its list fits.
- You act for the user with the rights they gave you. A call that fails with `insufficient_rights` includes an `approve_url`; show it to the user and ask them to open it, then try again.
- Prefer reading before changing. Say what you are about to change, and confirm before anything that deletes, archives or revokes.
- Lists are paged: pass `limit`, and `after` with the `next_cursor` of the previous page.

## Aliases and routes

An alias is a name apps send as `model`; its `route` picks the model that answers. A route is a tree of steps: `model` (with an optional `fallback` and the failures in `fall_back_on` that move on to it), `condition` (rules about headers, the API format, features, input tokens or the caller; `then` and `else`) and `split` (branches by weight, drawn per request, per caller or per value of a header).

- Read the catalogue first (`get_orgs_org_projects_project_models`): a route names models by their `model_…` id.
- Before you save a route, send it with a sample request to `post_orgs_org_projects_project_aliases_trace`. It sends nothing and shows the conditions, the split draws and every model the request would try, in order.
- Saving takes effect at once for every app that sends the alias. To change part of a route, read the alias, change its `route` and send the whole route back with `patch_orgs_org_projects_project_aliases_alias`.

## Usage, costs and latency

When the user asks for a statistic to look at, such as how much they spent this week or which model is slowest, call `show_usage_stats` with `metric` (`requests`, `tokens`, `cost`, `latency` or `errors`), `range` (`24h`, `7d` or `30d`) and `group_by` (`model`, `provider` or `key`), and `org` or `project` when there is more than one. Clients that support MCP Apps, such as Claude, show it as an interactive view the user can change themselves; you get a short summary either way, so answer from it instead of repeating the numbers in a table.

For anything else, go from wide to narrow. Each tool has a `get_orgs_org_projects_project_…` twin for one project.

1. `get_orgs_org_usage_span` gives the first and last day with usage.
2. `get_orgs_org_usage` sums requests, errors, tokens, cost and tool calls by `day`, `week`, `month`, `model`, `provider`, `key`, `member`, `route` or `project`. Its filters (`model`, `provider`, `key`, `member`, `route`, `project`) take a row's `key` as it is, and combine with each other and any grouping: `group_by=day`, `model=openai/gpt-5` and `key=key_…` is the daily usage of one model through one key.
3. `get_orgs_org_usage_timeline` reads the request log by `minute` (up to 2 days) or `hour` (up to 62 days), with the same filters plus `failed`. `group_by` (also `error_kind`) gives a bucket per step and group, and `top` keeps the busiest groups. Each bucket has latency percentiles in milliseconds, so it shows when something spiked and which model, key or error drove it.
4. `get_orgs_org_projects_project_logs` lists the single requests behind a bucket: its range as `from` and `to`, then `failed=true` or `min_duration_ms`. There a member is `user` or `service_account`, and `model` is the provider's model id, with `provider` apart.

Days and buckets are UTC. Costs are USD strings; `null` means you lack the right to see costs, not that nothing was spent. The timeline reaches back only as far as the organization keeps logs; use usage for older periods.
