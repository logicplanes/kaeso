---
name: kaeso
description: Use when the user asks about their Kaeso AI gateway, such as its organizations, projects, API keys, models, aliases and their routes, provider keys, limits, usage, costs, request logs or audit log, or wants to change any of them.
---

# Working with Kaeso

Kaeso is a self-hosted AI gateway. The `kaeso` MCP server gives you one tool per Platform API operation you are allowed to use, named after its operation id, such as `get_orgs_org_usage`.

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
