---
name: kaeso
description: Use when the user asks about their Kaeso AI gateway, such as its organizations, projects, API keys, models, aliases and their routes, provider keys, limits, usage, costs, request logs or audit log, or wants to change any of them. Also use it to connect Claude to their Kaeso install.
---

# Working with Kaeso

Kaeso is a self-hosted AI gateway. The `kaeso` MCP server gives you one tool per Platform API operation you are allowed to use, named after its operation id, such as `get_orgs_org_usage`, and `show_usage_stats`, which draws usage as a view.

## Connecting Kaeso

If the `kaeso` MCP tools aren't available, help the user connect their Kaeso install first. Most people doing this aren't developers, so keep it easy:

- Give one step at a time, in plain words, and wait until they say it worked before the next one.
- Name buttons exactly as they appear on screen, in bold, such as **Add custom connector**.
- Don't mention MCP, OAuth or other jargon unless they ask. Call it "connecting Claude to your Kaeso".
- When something goes wrong, ask what they see on screen and use **When something goes wrong** below.

### 1. Find the address

Every Kaeso install has its own address, the one the user opens to sign in to Kaeso, such as `https://kaeso.example.com`. Ask for it if you don't have it. Claude connects to that address followed by `/mcp`, such as `https://kaeso.example.com/mcp`.

The easiest way is often the Kaeso console itself: in Kaeso, they open their account menu, then **Account** › **Agents**, where the address is shown with an **Add to Claude** button that does steps 2 and 3 for them.

For claude.ai, the Claude apps and Cowork, Claude's servers connect to the install over the internet. The address must start with `https://` and open from any network. An address like `http://localhost:…`, `192.168.…`, or one that only works on an office network or VPN, can't be reached from there; Claude Code on their own computer can still use it. If that's their case, tell them so kindly and suggest Claude Code, or asking whoever runs their Kaeso to make it reachable over HTTPS.

### 2. Add Kaeso to Claude

Pick the steps for where they use Claude. Ask if you aren't sure.

**Claude on the web (claude.ai), the desktop and mobile apps, and Cowork.** You can't run commands here, so you guide them. A connector added once works in all of these, as long as they use the same Claude account.

1. Give them this link, with their address and `/mcp` percent-encoded after `connectorUrl=`, so `https://kaeso.example.com/mcp` becomes `https%3A%2F%2Fkaeso.example.com%2Fmcp`:
   `https://claude.ai/customize/connectors?modal=add-custom-connector&connectorName=Kaeso&connectorUrl=https%3A%2F%2Fkaeso.example.com%2Fmcp`
   Opening it in a browser where they're signed in to Claude shows **Add custom connector** with the name and address already filled in. Write the link out with their real address; don't leave the example in.
2. If the link doesn't open the form, they do it by hand: open **Customize** › **Connectors** (in the menu at their name, or the settings), choose **Add custom connector**, enter `Kaeso` as the name and their address with `/mcp` as the URL. They leave **Advanced settings** empty; Kaeso doesn't need a client ID or secret.
3. They choose **Add**. If they don't see **Add custom connector** at all, their plan or their organization may not allow custom connectors: on a Team or Enterprise plan, an owner adds Kaeso once under the organization's connector settings, and then everyone connects it for themselves.

**Claude Code.** Run `claude mcp add --transport http --scope user kaeso https://<their install>/mcp` with their address. `--scope user` makes it available in every project. If you can't run commands, give them the command to paste into their terminal.

### 3. Sign in and choose what Claude may do

- In claude.ai, the apps and Cowork, they choose **Connect** next to Kaeso. In Claude Code, they run `/mcp`, choose `kaeso` and **Authenticate**. If `/mcp` doesn't list `kaeso` yet, they restart Claude Code first.
- Their browser opens their Kaeso sign-in. After signing in, Kaeso asks what Claude may do: on the instance, in each organization and project, and on their account. Tell them they can start with only what they need and grant more later; Kaeso shows a link whenever Claude needs a right it doesn't have yet.
- They choose to allow it, and the browser returns to Claude.

### 4. Check that it works

- In claude.ai and the apps, they start a **new conversation**: tools load when a conversation starts. If Kaeso's tools don't show up, they open the tools menu below the message box (the **+** or the sliders icon) and make sure **Kaeso** is turned on.
- In Claude Code, `/mcp` shows `kaeso` as connected.
- Then call `get_me_memberships` and tell them which organizations you can see. That proves the connection end to end.

### When something goes wrong

- **"Couldn't connect" or "couldn't reach the server":** the address must end in `/mcp`, start with `https://` and open from any network (see step 1). Ask them to open their address without `/mcp` in a browser; it should show their Kaeso sign-in.
- **Nothing happens when they choose Connect:** their browser may have blocked the sign-in window; allow pop-ups for claude.ai and try again.
- **Signed in, but no tools:** start a new conversation, and check that **Kaeso** is turned on in the tools menu.
- **A tool says `insufficient_rights`:** Kaeso includes an `approve_url`. Give it to them; they open it, approve, and you try again.
- **It worked before and stopped:** someone may have disconnected Claude in Kaeso under **Account** › **Agents**. They choose **Connect** again in Claude's connectors, or run `/mcp` again in Claude Code.
- **They want to remove it:** in Claude, **Customize** › **Connectors**, then Kaeso's menu › **Remove**. In Claude Code, `claude mcp remove kaeso --scope user`. In Kaeso, **Account** › **Agents** shows every connected agent, and each can be revoked there.

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

Days and buckets are UTC. Costs are USD strings; `null` means you lack the right to see costs, not that nothing was spent. `converted_cost` is the same amount in the organization's `currency`, the one the console shows the user, so quote that. The timeline reaches back only as far as the organization keeps logs; use usage for older periods.
