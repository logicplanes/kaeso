---
name: kaeso
description: Use when the user asks about their Kaeso AI gateway, such as its organizations, projects, API keys, models, provider keys, limits, usage, costs, request logs or audit log, or wants to change any of them.
---

# Working with Kaeso

Kaeso is a self-hosted AI gateway. The `kaeso` MCP server gives you one tool per Platform API operation you are allowed to use, named after its operation id, such as `get_orgs_org_usage`.

- Start with `get_me_memberships` to find the user's organizations. Ids look like `org_…` and `proj_…`.
- Every tool takes `agent_model`: the model family you are, such as `claude`, or `unknown` if none in its list fits.
- You act for the user with the rights they gave you. A call that fails with `insufficient_rights` includes an `approve_url`; show it to the user and ask them to open it, then try again.
- Prefer reading before changing. Say what you are about to change, and confirm before anything that deletes, archives or revokes.
- Lists are paged: pass `limit`, and `after` with the `next_cursor` of the previous page.
