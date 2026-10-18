---
title: Platform API
description: Manage an install over HTTP, as the console does.
order: 3
---

The Platform API manages organizations, projects, members, keys, provider keys, models, limits, usage and logs. It lives under `/platform/v1` on the same domain as the model API. The console uses nothing else, so a script can do everything the console does.

The full reference is the OpenAPI document [`openapi.json`](https://github.com/logicplanes/kaeso/blob/dev/openapi.json), generated from the gateway's code. Every operation in it names the one right it checks. To get the document of the version you run:

```bash
docker compose exec gateway kaeso openapi > openapi.json
```

[Automate with the Platform API](automation.md) sets up a project, a model and an app's key with curl, using a platform key, a project key or a session.

## Authentication

| Method | Send | Acts with |
| --- | --- | --- |
| Session | The session cookie that the sign-in endpoints under `auth/*` set | The signed-in user's rights |
| API key with platform access | `Authorization: Bearer sk-kso-…` | Its member's or service account's rights, in its own project only |
| Platform key | `Authorization: Bearer sk-kso-org-…` | The Admin role's rights, in its organization and every project in it |

Sessions serve the console and people with curl. The cookie is the HttpOnly `__Host-kaeso_session`, or `kaeso_session` on an `http://` install at a loopback address such as localhost. Project keys suit scripts and CI for one project, platform keys automation for a whole organization.

- **Keys reach `orgs/{org}/*` only.** `me/*`, `instance/*` and the signed-in routes of `auth/*` refuse them with 403, and so does every organization-level route for a project key. The Platform API reads keys from `Authorization` only, never from `x-api-key`.
- **Platform access is chosen when a key is created**, with `"access": "platform"` or `"both"`; the default is `inference`. It needs the right `keys.platform_access.grant`. See [API keys](../console/api-keys.md).
- **Platform keys are created only in a session**, by someone who holds every right of the Admin role. They never call models.
- **Organization policies apply to sessions.** An organization that requires single sign-on refuses other sessions with 403; one that requires two-factor authentication refuses password sessions without a second factor.
- **Unknown keys are throttled.** Too many requests with unknown API keys from one address, on the model API and the Platform API together, get `429 rate_limited` with `Retry-After` until the window ends; [API key failures](../self-hosting/configuration.md#api-key-failures) has the settings.

### Cross-site requests

Every request other than `GET`, `HEAD` and `OPTIONS` that carries the session cookie, and every such request to a public `auth/*` endpoint, must send `Origin` with the origin of the install's public URL, such as `Origin: https://kaeso.example.com`. Browsers send it themselves; `Referer` doesn't count in its place. Without it, or with another origin, the answer is `403 permission_denied`, and the message names the header to send. This keeps other sites from acting with a person's session or signing a browser in to an account of theirs.

- Requests with an API key or platform key need no `Origin`.
- Reads never need it, `GET auth/methods` and `GET auth/oidc/start` included.
- A server that calls `POST auth/methods`, `POST auth/password` or `POST auth/oidc/start` sends the install's origin. To send people from another site to an identity provider, link to `GET auth/oidc/start` instead; see [sign-in and single sign-on](../self-hosting/sign-in.md#links-that-go-straight-to-the-provider).

### Recent sign-in

In a session, some actions need a sign-in within the last 15 minutes; otherwise the answer is `403 recent_sign_in_required`, and the console asks to sign in again. Keys are never asked.

- Operations guarded by a right flagged `recent_sign_in` in `orgs/{org}/rights`: `org.delete`, `org.ownership.transfer`, `provider_credentials.create`, `provider_credentials.replace`, `security.sso.manage`, `security.sso.require`, `platform_keys.create`, `logs.delete` and `keys.platform_access.grant`.
- In your account: changing your email or notifications, turning off two-factor authentication, new recovery codes, deleting the account.
- For instance admins: changing instance settings, and scheduling, restoring or deleting an organization.

## Conventions

| Topic | Rule |
| --- | --- |
| Format | REST with JSON. Paths in plural kebab case (`service-accounts`), fields in `snake_case`. Bodies are sent with `content-type: application/json`; a field the endpoint doesn't know is refused with 400 |
| Ids | A prefix and a time-ordered part: `org_…`, `proj_…`, `usr_…`, `key_…`, `opk_…`, `svc_…`, `cred_…`, `model_…`, `alias_…`, `lim_…`, `role_…`, `inv_…`, `ses_…`, `req_…`, `evt_…` |
| Times | RFC 3339, in UTC |
| Money | US dollars as decimal strings, such as `"0.0001"`. Prices and amounts are sent as decimal strings or whole numbers; a JSON fraction such as `0.5` is refused with 400 |
| Limit amounts | At most 10^12 requests or tokens, or 1,000,000,000 USD for a spend budget |
| Create | `POST` on the collection, answered with 201 and the resource |
| Update | `PATCH` with only the fields to change |
| Actions | `POST …/archive`, `…/disable`, `…/enable`, `…/replace`, `…/restore`; archiving keeps the record |
| Delete | `DELETE`, only where something is really deleted: provider keys, aliases, catalogue models, limits, custom roles, sessions, invitations, the account |
| Audit | Every change is written to the audit log of its level, in the same transaction |
| Request ids | Every answer carries `kaeso-request-id` |

### Lists

A list takes `limit` (1 to 100, default 20) and `after`, the `next_cursor` of the previous page; filters are further query parameters. Lists are ordered by id, which is creation order; request logs and audit logs come newest first.

```json
{ "data": [], "has_more": true, "next_cursor": "…" }
```

`next_cursor` is `null` on the last page.

### Revisions

Catalogue models and limits carry a `revision` that goes up with every change, and reading, creating or changing one answers with `ETag: "<revision>"`. Send that tag back in `If-Match` with `PATCH` or `DELETE` to change only the revision you read: if the entry changed since, the answer is `412 conflict`. Without `If-Match`, the change applies to the current revision.

### Organizations scheduled for deletion

An organization scheduled for deletion is read-only until it is restored or deleted. Reads work as the caller's rights allow. Every change in it, leaving it and accepting an invitation to it included, is `409 org_scheduled_for_deletion`, except `POST orgs/{org}/restore` and the instance admins' `instance/orgs/{org}/*`. Its keys don't authenticate at all.

`auth/session` shows such an organization in each membership with `org_state`, `org_deletion_due_at`, and rights reduced to seeing, exporting and `org.delete`. It also carries the instance's `org_deletion` policy (`allowed`, `grace_days`).

## Errors

```json
{
  "error": {
    "type": "permission_denied",
    "message": "You need the right `models.manage` in this project; ask one of its admins for it.",
    "right": "models.manage"
  }
}
```

The message is one short sentence for people; the console shows it as it is. `param` names the body field, query or path parameter at fault, and `right` the right the caller lacks when that one right would allow the request; each is left out when it doesn't apply.

| Status | `type` | When |
| --- | --- | --- |
| 400 | `invalid_request` | A body that isn't JSON, an unknown field, a parameter of the wrong type or form, a value out of range |
| 401 | `authentication` | No session or key, or one that is unknown, expired or ended |
| 403 | `permission_denied` | A missing right, a key on a route it can't use, a missing `Origin`, an organization policy |
| 403 | `recent_sign_in_required` | See [recent sign-in](#recent-sign-in) |
| 404 | `not_found` | An unknown path, or something that doesn't exist or the caller can't see |
| 405 | `invalid_request` | A method the path doesn't take; `Allow` lists the ones it does |
| 409 | `conflict` | The change clashes with the current state, such as a name already taken |
| 409 | `org_scheduled_for_deletion` | A change in an organization waiting to be deleted |
| 412 | `conflict` | `If-Match` names an older revision |
| 413 | `request_too_large` | The body is larger than `KAESO_BODY_LIMITS_PLATFORM_API`, 1 MiB by default |
| 429 | `rate_limited` | Too many unknown keys, or a limit on a Playground request; `Retry-After` says when to try again |
| 500 | `internal` | A fault in the gateway; the message names the request id to report |
| 502, 503, 504, 429 | `provider_error` | Only for the Playground's model requests |
| 503 | `unavailable` | A database or other backing service can't be reached, such as "Kaeso cannot reach its database right now; try again shortly." |

## Areas

| Paths | What |
| --- | --- |
| `auth/*` | First-start setup, sign-in methods, password and two-factor, sign-in links, password reset, single sign-on, sessions, sign-up where the instance allows the address, invitations |
| `me`, `me/*` | Your profile, picture and language (`locale`: `auto`, `en`, `de`, `zh-CN`), email, password, two-factor and recovery codes, sessions, memberships, your keys, notifications, creating an organization, deleting the account |
| `orgs/{org}/*` | The organization and its picture, members, invitations, roles and rights, projects and the default one, provider keys (several per provider, a default, assignments to projects), security (single sign-on, required single sign-on and two-factor, key lifetime), platform keys, log settings, storage and deletions, usage, the audit log, ownership transfer, deletion and restoring |
| `orgs/{org}/projects/{project}/*` | The project and its picture, members and their model access, service accounts, keys with their models and limits, key policy, the catalogue (search, preview, batch) and aliases, limits, content logging, request logs and exports, usage, the audit log, the [Playground](#playground) |
| `instance/*` | For instance admins: users, their invitations, sessions and admin flag; organizations with their members, projects, usage and storage; settings with a test email; the models.dev status and sync; system health, storage and log buffer; updates: the newest releases, their settings, applying one through the updater and its progress; the instance audit log |

Instance admins see every organization's metadata, never its content. They create, rename, schedule for deletion, restore and delete organizations under `instance/orgs`. `POST instance/orgs/{org}/owner` makes any active user an owner, the admin included; that is how an instance admin gets in to manage an organization, and it is recorded in the organization's audit log. Only instance admins reset a user's two-factor authentication, with `POST instance/users/{user}/reset-2fa`.

There are no billing routes. The rights `billing.view` and `billing.manage` are for Kaeso Cloud and carry the flag `cloud_only` in `orgs/{org}/rights`.

## Roles and rights

Every organization member has one organization role, and every project member and service account one project role, each with rights added or removed individually. A role is built in or a custom role of the organization (`role_…`). An organization role also holds rights in every project. [Rights](../concepts/rights.md) lists every right and built-in role.

| Route | Right | What |
| --- | --- | --- |
| `GET orgs/{org}/rights` | `org.view` | Every right by category, with its name, description, flags (`sensitive`, `recent_sign_in`, `cloud_only`) and the rights it needs |
| `GET orgs/{org}/roles`, `GET …/roles/{role}` | `org.view` | Built-in and custom roles with their rights, rights in every project and number of holders; `?scope=org` or `project` |
| `POST orgs/{org}/roles`, `PATCH …/roles/{role}`, `DELETE …/roles/{role}` | `roles.manage` | Create, change and delete custom roles, at most 50 per organization |
| `GET orgs/{org}/members/{user}/rights` | `members.view` | A member's effective organization rights, rights in every project, and rights in each of their projects the caller can see, each with its sources |
| `GET orgs/{org}/projects/{project}/members/{user}/rights` | `project.members.view` | A member's effective rights in this project, with sources |
| `GET orgs/{org}/projects/{project}/my-rights` | `project.view` | The caller's own rights in this project, with sources |
| `GET orgs/{org}/rights/{right}/grantors`, `GET …/projects/{project}/rights/{right}/grantors` | `members.view`, `project.view` | Whom to ask for a right: up to 10 active members who hold it and may change rights, owners first |
| `GET orgs/{org}/projects/{project}/audit-log` | `project.audit_log.view` | The changes made in the project |

A grant is sent as `{"role": "developer", "rights_added": [], "rights_removed": []}`, where `role` is a built-in role's name or a custom role's id; `preset` is its deprecated name. It comes back with `role` (`{id, kind, name}`), the effective `rights` and, for organization grants, `project_rights`. Invitations take `role` and `project_roles: [{project_id, role}]`; the single sign-on setup takes `default_role`.

Every change of rights, to a grant or a custom role, follows the [grant rules](../concepts/rights.md#grant-rules). Giving a right the caller doesn't hold, or changing a member who holds a right the caller lacks, is refused with 403, such as "You can't grant `audit_log.view`: you don't hold it yourself." Giving a right without one it needs, or removing one that another right needs, is refused with 400, and the message names both, such as "`logs.delete` needs `logs.storage.view`; remove `logs.delete` too."

The sources of an effective right are `owner`, `role` (the organization role, for organization rights), `project_role`, `added`, `org_role` (the organization role's rights in every project), `org_right` (an organization right that reaches into projects, such as `projects.view.all`) and `key` (an API key acting in the project).

## Playground

The console's [Playground](../console/playground.md) sends requests to the project's models as the signed-in member. Both of its routes need a session and the project right `playground.use`; keys are refused with 403, since apps call models on `/v1`.

`GET orgs/{org}/projects/{project}/playground/models` lists the models the member can call there: the catalogue without embeddings models, narrowed by the member's model access. Each comes with the name to send, its aliases, whether it reasons and with which efforts, whether it takes `temperature`, its limits and prices, and whether the project has a provider key for it.

`POST orgs/{org}/projects/{project}/playground/responses` takes an OpenAI Responses request with `model`, `input` (a string or input items), `instructions`, `temperature`, `top_p`, `max_output_tokens`, `reasoning.effort`, `reasoning.summary`, and `"stream": true`, which is required. The answer is the Responses event stream that `/v1/responses` sends, with keepalive comments. With a [session](automation.md#use-a-session-instead):

```bash
curl -N -b "$JAR" -H "Origin: $KAESO" -H 'content-type: application/json' \
  -d '{"model": "claude-haiku-4-5", "input": "Hello", "stream": true, "reasoning": {"summary": "auto"}}' \
  "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/playground/responses"
```

A Playground request is a model request like any other, made by the member without a key:

- **Same path.** The model is resolved in the project's catalogue, aliases and `provider/model` included, and checked against the member's model access. The project's provider key is used, and closing the stream cancels the provider's request.
- **Limits.** The member's and the project's rates and budgets apply; key limits don't, as there is no key.
- **Usage and logs.** The request is charged to the project and counted as the member's; usage by `key` shows it as `playground`. It is logged with the source `playground`, with its content when the project logs content; `logs` and `logs/export` filter with `source=api` or `source=playground`.

Errors before the stream starts are Platform API errors:

| Status | `type` | Cause |
| --- | --- | --- |
| 400 | `invalid_request` | `param` is `model` for a model outside the catalogue or one the provider doesn't serve to the project's provider key, and `stream` for `"stream": false` |
| 403 | `permission_denied` | The model is outside the member's model access |
| 409 | `conflict` | The project is archived, or no provider key is set up for the model's provider |
| 409 | `org_scheduled_for_deletion` | The organization is waiting to be deleted |
| 429 | `rate_limited` | A rate or budget, named in the message |
| 503 | `unavailable` | The catalogue, the provider keys or the limits can't be read, or the gateway is restarting |
| 502, 429, 503, 504 | `provider_error` | The provider failed or refused the provider key, rate limited the request, is overloaded, or timed out |

Once the stream has started, errors arrive as Responses `error` events, as on `/v1`.

## How the console uses it

The console is served on the gateway's own origin, so its session cookie is first-party and it needs no CORS. It keeps only the HttpOnly cookie and sends `Origin` with every change, as browsers do. Beyond that, it:

- reads `auth/session` for the user, their memberships with organization role, rights and rights in every project, whether they are an instance admin, and whether they may create organizations (`can_create_orgs`);
- reads a project's `my-rights` to decide what to show there, and takes right names, groups and dependencies from `orgs/{org}/rights`;
- fills the model forms from `models/search` and `models/preview`;
- shows `recent_sign_in_required` as a prompt to sign in again, then repeats the action;
- downloads `logs/export` and `audit-log/export` as streamed files;
- streams Playground replies with `fetch`, where **Stop** aborts the fetch and so cancels the provider's request;
- charts usage by day up to about three months, by week up to about a year and by month beyond, and starts **All time** at the `first_day` of `usage/span`.
