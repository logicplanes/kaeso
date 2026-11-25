---
title: Organizations and projects
description: Create organizations and projects, connect single sign-on, and manage their settings.
order: 1
---

An organization holds members, provider keys and projects. A project holds a model catalogue, API keys, service accounts, limits and request logs. People join an organization with an organization role, and each of its projects with a project role.

## Create an organization

Open the menu at the foot of the sidebar and choose **Create organization**. You become its owner, and it starts with no projects. The install decides who may create organizations ([instance settings](instance-administration.md#change-install-settings)).

The same menu lists your organizations and switches between them. Opening Kaeso, or another organization, shows the organization's **Overview**; open a project from there or from the sidebar.

## Read usage

The organization's **Overview** is its home. It leads with the release Kaeso runs, then shows requests, tokens, cache hits and failed requests over the last 7, 14 or 30 days, and with the right to see costs the month's spend split by project. Below are the projects with their requests today, the members, provider keys, MCP servers, recent changes and the newest models in the catalogue. Each total opens **Analytics** at that metric.

**Analytics** shows requests, tokens, cost, errors and tool calls for a period you choose, from today to all time, and compares each total with the period before it, except for today, which isn't over yet, and all time. The totals lead, and the one you choose is charted against the period before, drawn as a dashed line: by the hour for a day or two when request logs are on, by UTC day up to a quarter, by week up to two years and by month beyond. Under the chart are the five projects and models with the most of that total, each with its share and its change; **Reliability**, with the failed requests, their kinds and the slowest hours; **Efficiency**, with the cache hit and the tokens a request uses; and **Spend**, with the month's forecast and the cost by project. The kinds of failure and the slowest hours come from the request logs, for periods of up to 62 days. A day, week or month that is still running is drawn dashed or lighter, and its tooltip says "so far". **Projects**, **Models** and **Members** break the period down one row each, with its share, what grew and what shrank, its error rate, tokens, cache hits and cost. At the foot of its **Overview** tab, a breakdown groups the period by project, model or member, or **By key that paid**: the provider key each request was last sent with, and **No provider** for requests sent to none. Seeing usage needs the right **View org usage**, and seeing cost **View org costs**. **Share** makes a read-only link to the page for people without an account ([share usage](share-usage.md)). Cost is in the organization's currency, at each day's ECB rate ([currencies](../concepts/currencies.md)).

A project's **Overview** shows its activity over the last days, its latest requests as they arrive, its limits with what is used, its models by use, its keys by last use, its members and its recent changes. Its **Analytics** shows the same as the organization's for one project, with **Models**, **Keys** and **Members**, the project's spend budget beside the forecast, and a breakdown by time, model, key, member or route at the foot of the page. A row of a list opens the request logs of the period with that model, key or kind of failure.

A route is the app's format, the format Kaeso sent to the provider, and the provider. Its breakdown has a **Cache hit** column: the share of the input read from the prompt cache. The same model can hit the cache through one route and never through another, so **By route** shows where caching fails. Requests from before Kaeso recorded routes show as **Route not recorded**.

**By client** splits a project's usage by the client app that sent it, named from the start of its user agent: Codex, Claude Code, the OpenAI and Anthropic SDKs for Python and Node.js, the AI SDK and curl, any other as **Other client**, and requests without a user agent apart. It reads the request logs, so it's offered for periods of up to 62 days, and a client's row opens its requests.

A breakdown with media requests has a **Media** column: the images made, the characters spoken, the length of audio made or transcribed, and the videos made with their length. A group whose requests had no known price shows its cost as **Unknown**; one where only some had none shows its cost with how many requests are unknown.

In both breakdowns, a number column's header sorts the rows, with what isn't known last, and **Download CSV** saves the rows as shown, in their order, with each row's id next to its name; the file is made in your browser. In a project's breakdown, a row opens the requests it counts in the request logs, in the period's UTC days: a day, week or month opens its own days; a model, key, member or route opens its requests, and the **Playground** row the Playground's. A route that wasn't recorded opens nothing. Rows open only for someone who may read the request logs.

## Change organization settings

Open **Organization settings** in the menu at the foot of the sidebar. It has these pages:

| Page | What it holds |
| --- | --- |
| General | Picture, name, currency and organization id, **Transfer ownership**, and deleting the organization. **Currency** is what the organization shows spend, prices and budgets in ([currencies](../concepts/currencies.md)) |
| Members | Members, invitations and roles ([members and roles](members-and-roles.md)) |
| Security | Single sign-on, provisioning, the sign-in requirements, the maximum key lifetime and members' agents |
| Guardrails | Services and built-in rules that check requests and replies ([guardrails](guardrails.md)) |
| Platform keys | Keys for scripts that manage the organization ([platform keys](api-keys.md#platform-keys)) |
| Webhooks | Where the organization's notices go, signed, such as on-call tools and chat ([webhooks](webhooks.md)) |
| Shared links | The organization's read-only links and who may open them ([share usage](share-usage.md)) |
| Logging | Content logging, retention, storage and log deletion ([request logs](request-logs.md#retention)), the [response cache](response-cache.md) and [stored responses](#stored-responses) |
| Audit log | Every change in the organization ([audit log](#read-the-audit-log)) |

A picture is a PNG, JPEG or WebP image, and the console crops it to a square.

## Single sign-on

Single sign-on lets members sign in through your OpenID Connect identity provider. First create an app for Kaeso at your provider. Then, under **Security**, choose **Connect an identity provider** and answer five steps:

1. **Where is your identity provider?** The step shows the **Redirect URL** to register in the app as its sign-in callback. Enter the app's issuer URL, such as `https://login.example.com`.
2. **Which app signs people in?** Enter its client id and client secret.
3. **Which email domains sign in with it?** The sign-in page sends people with an address at these domains to your provider. Internationalized domains work in either form, `bücher.example` or `xn--bcher-kva.example`.
4. **Can people join without an invitation?** Choose **Invite only**, or the role that people new to Kaeso join with. That role can't be Owner.
5. **How does the sign-in page offer it?** Enter the **Name on the button**, such as `Acme ID` for "Continue with Acme ID". Left empty, the button says "Continue with SSO". With **Start without a click**, the sign-in page goes on to your provider without showing its button. It does so when someone enters an address at your verified domains and the provider is their only way in. **Require verified addresses** refuses tokens that don't mark the address as verified; turn it off only for a provider that never sends `email_verified`.

Once you connect, the page shows the **Redirect URL**: register it at your provider as the sign-in callback. To send members to single sign-on from your intranet or a bookmark, link to your install's sign-in page, such as `https://kaeso.example.com/sign-in`. It asks for their address first ([the sign-in page](../self-hosting/sign-in.md#the-sign-in-page)).

### Verify your email domains

Single sign-on signs in and admits only addresses at verified domains, so no organization can claim a domain it doesn't hold. For each domain, **Security** shows whether it is **Verified** or **Unverified**, and the TXT record an unverified one needs. Add it at your DNS provider, then choose **Verify** next to the domain:

```text
_kaeso-verify.acme.example  TXT  "kaeso-verify=Qm9ZtX3c8L1aV7pK2rN5wE0yH4uJ6sD9fG1bC3xT8eA"
```

DNS changes can take a while to show; choose **Verify** again later. Public mail domains, such as `gmail.com`, can't be added, and neither can a domain another organization verified. For a domain that public DNS doesn't answer for, an operator verifies it on the server with `kaeso sso verify-domain acme.example`.

**Edit single sign-on** changes the connection. An empty client secret keeps the stored one while the issuer and client id stay the same. A new issuer or client id ends the sessions the previous provider signed in, and the agents and portal clients approved in them. **Disconnect** removes it and ends the sessions it signed in, with their agents and portal clients, and members sign in with a password or a sign-in link again.

Single sign-on signs in an existing Kaeso account only once its owner links it to your provider, from their **Account** ([link single sign-on](account.md#link-single-sign-on)). The account must also belong to the organization: invite the person, and they accept by signing in another way. Changing single sign-on needs a console session with a recent sign-in; platform keys can't. [Sign-in and single sign-on](../self-hosting/sign-in.md) has every rule, and how to run an install on single sign-on alone.

Members confirm a recent sign-in with a password or a sign-in link. Single sign-on confirms only for accounts with no other way, such as the ones the organization manages while **Require single sign-on** is on. That works only if your provider signs people in afresh when asked and sends `auth_time` in its ID tokens.

When a confirmation comes back without a fresh sign-in, **Security** shows **Recent sign-in**, and the provider isn't offered for confirming again. With **Require single sign-on** on, the accounts it manages then can't confirm at all ([recent sign-ins](../self-hosting/sign-in.md#recent-sign-ins)).

Once connected, the organization manages the accounts at its verified domains: a sign-in through its provider opens everything those accounts can open, except instance administration ([managed accounts](../self-hosting/sign-in.md#managed-accounts)).

### Provision members from your identity provider

Under **Security** › **Provisioning**, **Create token** lets your identity provider add and remove members through SCIM: Okta, Microsoft Entra ID and other providers that speak SCIM 2.0. A member the provider deactivates leaves the organization and its projects, and their keys in it are archived, as with **Remove from organization**. The section shows the base URL, when the provider last called, how many members it keeps active and the last error ([SCIM provisioning](../api/scim.md)).

### Roles and projects from your provider's groups

Under **Security** › **Single sign-on** › **Groups**, rules give the people who sign in through your provider an organization role and project memberships from their groups. This needs the **Manage SSO** right and a console session with a recent sign-in.

- **Groups claim** names the ID token claim that lists the person's groups, `groups` by default. **Extra scopes** are scopes Kaeso asks for besides `openid email profile`, such as `groups` for providers that send the claim only on request.
- Each rule names a group's value, compared exactly, with an optional organization role and projects with a role each. Up to 200 rules. The first matching rule sets the organization role; without a match the role people join with applies. Every matching rule adds its projects. A project whose custom role was deleted since gives nothing until you change the rule.
- A rule can't make anyone an owner, and an owner is never changed by one. A rule gives only roles whose rights you hold.
- **Add only** never takes anything away. **Keep in sync** also takes away what a rule gave once no rule gives it: the role goes back to the one people join with, and a project membership ends, with the person's keys in that project archived. What an admin gave by hand always stays.

The rules apply at each sign-in through your provider, so a change in a group applies at the person's next sign-in. A token without the claim changes nothing, and **Security** shows when that last happened, so a claim broken at the provider can't take access away. **Groups in your last sign-in** lists the values your own token carried, to copy the exact strings into rules.

Okta sends groups once the app has a groups claim with a filter, such as **Matches regex** `.*`. Microsoft Entra ID sends group object ids, not names. For a person in more than 200 groups, it sends a reference instead of the list. Kaeso then reads the person's groups from Microsoft Graph with the sign-in's access token. That needs the delegated `User.Read` permission and no scope of another API under **Extra scopes**; if Graph refuses, the sign-in changes nothing. To map app roles instead of groups, set **Groups claim** to `roles`, and rules match your app roles' values.

Graph lists every group and directory role the person is in, nested ones included. With **Security groups**, the token leaves out distribution lists and Microsoft 365 groups, but with `User.Read` Kaeso can't tell them apart in Graph. A rule on one of them would apply only to people in more than 200 groups, so name security groups and directory roles in rules.

## Require single sign-on or two-factor authentication

Under **Security**, you can tighten how members get into the organization:

| Setting | What it does |
| --- | --- |
| Require single sign-on | Accounts at its verified domains sign in only through SSO; guests keep their own sign-in. The accounts it manages can't sign in with a password or a sign-in link anywhere on the install, and their other sessions end, with the agents and portal clients approved in them. It needs a connected provider |
| Require two-factor authentication | Members need a sign-in with a code from their authenticator, with a password or a sign-in link. A sign-in through single sign-on is enough too, from the organization's own provider or the one that manages the member's account |
| Maximum key lifetime | The longest a new key may live, in days, or **No limit**. Existing keys keep their expiry, and a project can set a shorter limit |

Turning on **Require two-factor authentication** asks first and names the active members without an authenticator app whom it would lock out, leaving out accounts the organization manages while it requires single sign-on.

Turning on a requirement you don't meet yourself closes the organization to you too, until you sign in through the provider or set up two-factor authentication. The console asks before it saves. Requiring single sign-on keeps your own session, so a [sign-in link from the server](../self-hosting/sign-in.md#get-back-in) is never your only way back. Disconnecting the identity provider turns off **Require single sign-on** as well.

Kaeso emails the members a new requirement affects, unless they turned off **A sign-in rule affects you**: for two-factor authentication, the active members without an authenticator app, apart from the accounts the organization manages while it requires single sign-on; for single sign-on, the accounts it manages, and those not linked to its provider yet are told to ask an admin for help.

## Let members' agents in

Members can connect agents, such as Claude Code or Codex, that act for them through the [MCP server](../api/mcp.md). Under **Security**, the **Agents** section holds the organization's agent policy. It decides whether those agents may act in the organization, and how far:

| Setting | What it does |
| --- | --- |
| Allow members' agents | On by default. Turned off, every agent gets 403 in this organization at its next request and keeps working elsewhere |
| Most an agent may do | **Observer**, **Builder**, **Operator** or **Full access**, each including the ones before it. **Full access** is the default. An agent never gets more here, whatever its member gave it |

Changing the policy needs **Change member roles** and a console session; no agent can change it. [Agents](../concepts/rights.md#presets) lists what each preset holds.

**Agents with access**, under it, lists the agents that may act in the organization, with their member, model family, and when they were created and last used. Seeing them needs **View members**. **Close access** closes the organization to an agent and needs **Change member roles**. The agent keeps working in its member's other organizations, and its member can open this one to it again.

## Delete an organization

Under **General**, choose **Delete organization** and type the organization's name to confirm. You need the right **Delete organization**, which owners have, and the install must allow deleting organizations.

1. Every API key of the organization stops working at once, and Kaeso emails the members.
2. For a grace period, 7 days unless the install sets another, the organization is read-only and every page shows a banner. An owner can still choose **Restore organization**.
3. After the grace period, Kaeso deletes its projects, keys, provider keys, settings, usage and logs for good. Once the grace period has ended, the organization can't be restored any more, even before Kaeso has finished deleting it.

## Read the audit log

The organization's **Audit log**, in **Organization settings**, records every change in the organization. A project's **Audit log**, in the project's sidebar, records every change in that project.

Each entry shows when it happened, who made the change, the action and its target. Filter by actor, action, target id and days. Choose an entry to see the IP address, user agent and request id, any change of rights, and the values before and after. A target that still exists shows its name, and opens with a click when you may see it: in a project, a member, key, model, alias, service account, limit, tool grant or the project's settings. A deleted target keeps the name it had, without a link.

An agent's change shows the picture of the member it acts for, with a small bot badge at its lower right. Hover over it to see the member, the agent and its model family. The **Agents** filter shows only the changes agents made, and the Platform API names the agent in `actor.agent` ([activity and audit](../api/mcp.md#activity-and-audit)).

A member's sheet opens the audit log at their changes with **Their changes**, and at the changes made to them with **Changes to them**, for people who may read that log: under **Members** in **Organization settings** the organization's, in a project its own.

| To | Right |
| --- | --- |
| Read the organization's audit log | **View audit log** |
| Export it as JSON Lines, with the current filters | **Export audit log** |
| Read a project's audit log | **View project audit log** |

## Create a project

On **Projects**, choose **Create project** and give it a name. You become the project's admin. Creating a project needs the right **Create projects**.

**Archived** lists archived projects, and **All** every project. Search finds a project by name, and each project shows its requests in the last 30 days to people who can see the organization's usage. For them, **Sort** puts the projects with the most requests first, or, with **View org costs**, the ones with the highest cost; projects without requests or cost come last, by name. The address keeps the order, so you can share it.

All of an organization's projects are alike. People who join the organization join no project by it: an invitation names the projects they join, and anyone else is added on a project's **Members** page.

## Change project settings

A project's **Settings** page holds these settings:

| Setting | What it does |
| --- | --- |
| Picture, project name | Shown in the sidebar and in request logs |
| Project id | The id the Platform API uses |
| Maximum key lifetime | **Use the organization's limit**, or a shorter maximum for new keys in this project |
| Content logging | Whether request and reply bodies are kept with their log entries. It can't be on while the organization has it off |
| Keep media | Whether logged content keeps its images, audio and files ([request logs](request-logs.md#media-in-requests-and-replies)). It can't be on while the organization keeps none |
| Retries | How many times, from **Never** to 3, a request that names a model, or an alias step without its own, tries the model again after a dropped connection, a 5xx or an overloaded provider ([alias routes](../concepts/alias-routes.md#fallbacks)) |
| Response cache | Whether repeated identical requests get a stored reply, and for how long ([response cache](response-cache.md#turn-the-cache-on)) |
| Guardrails | The organization's guardrails this project's requests pass, in order ([guardrails](guardrails.md#choose-the-projects)) |
| Allow responses stored at the provider | Whether Responses apps may let the provider keep their responses and continue them ([stored responses](#stored-responses)). It can't be on while the organization doesn't allow it |
| Model retirement | Whether aliases move to a retired model's replacement on its shutdown day, and whether requests to it get the replacement ([model lifecycle](../concepts/model-lifecycle.md)) |
| Archive project | Archives the project with its keys and service accounts, for good. Its keys stop working at once |

Kaeso archives projects and never deletes them, so their usage and logs stay. Any project can be archived.

## Stored responses

An app that speaks OpenAI's Responses format can ask OpenAI to keep a response and continue the conversation later with `previous_response_id`, instead of sending the whole conversation each time ([conversation state](https://developers.openai.com/api/docs/guides/migrate-to-responses)). OpenAI then keeps the conversation for 30 days. That is a decision about where your data lives, so it is off until both switches are on:

1. An organization admin opens **Organization settings**, then **Logging**, and turns on **Allow responses stored at the provider**. This needs the **Edit organization** right. Turning it off stops it in every project.
2. A project admin turns on **Allow responses stored at the provider** in the project's **Settings**.

Then, for requests in the Responses format to an OpenAI model with an OpenAI API key:

- Kaeso records each response OpenAI keeps for the project: unless the request sends `store: false`, since OpenAI keeps responses by default. It keeps only a hash of the response's id, with the project, the provider key and the model.
- `previous_response_id` continues a response only in the project that stored it, with the provider key that stored it. Another project gets `404` as if the response didn't exist, even when both projects share the provider key.
- `GET /v1/responses/{response_id}` reads a stored response and `DELETE /v1/responses/{response_id}` deletes it at OpenAI, with the same checks.
- Limits reserve the earlier conversation's tokens too, since OpenAI bills them as input, and the response cache lets these requests pass.

Kaeso refuses `store: true` and `previous_response_id` with `400`, naming the field, when either switch is off, when the model is called in another format or at a provider that keeps no responses, or when the project's key is an account sign-in such as a ChatGPT plan; when a switch is off, Kaeso sends `store: false` so OpenAI keeps nothing by default. An alias moves on from a step that can't continue the response to its next step. `conversation`, `background: true` and `item_reference` items stay refused everywhere, and `/v1/responses` batch files follow the same rules. Turning the switches off leaves the responses OpenAI already keeps there until they expire; delete them first if you need to.

## Next steps

- [Members and roles](members-and-roles.md): Invite people and give them roles in the organization and its projects.
- [Provider keys](provider-keys.md): Connect the model provider keys your projects call models with.
- [Request logs](request-logs.md): Choose what the organization keeps, and for how long.
