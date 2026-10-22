---
title: Platform API
description: Authenticate, page through lists, handle errors and find the routes to manage an install over HTTP.
order: 3
---

The Platform API manages everything the console shows: organizations, projects, members, keys, provider keys, models, limits, usage and logs. It lives under `/platform/v1` on your install's domain, and the console uses nothing else.

List an organization's projects with a platform key in `KAESO_PLATFORM_KEY`, and your organization's id in place of `org_…`:

```bash tab="curl"
curl https://kaeso.example.com/platform/v1/orgs/org_…/projects \
  -H "Authorization: Bearer $KAESO_PLATFORM_KEY"
```

```python tab="Python"
import json
import os
import urllib.request

request = urllib.request.Request(
    "https://kaeso.example.com/platform/v1/orgs/org_…/projects",
    headers={"Authorization": f"Bearer {os.environ['KAESO_PLATFORM_KEY']}"},
)
with urllib.request.urlopen(request) as response:
    print(json.load(response))
```

```ts tab="TypeScript"
const response = await fetch("https://kaeso.example.com/platform/v1/orgs/org_…/projects", {
  headers: { Authorization: `Bearer ${process.env.KAESO_PLATFORM_KEY}` },
});
console.log(await response.json());
```

The answer is a page of projects:

```json
{
  "data": [
    {
      "id": "proj_…",
      "org_id": "org_…",
      "name": "Support bot",
      "is_default": false,
      "content_logging": true,
      "created_at": "2026-10-01T09:30:00Z"
    }
  ],
  "has_more": false,
  "next_cursor": null
}
```

[Automate with the Platform API](automation.md) sets up a project, a model and an app's key step by step.

## OpenAPI document

[`openapi.json`](https://github.com/logicplanes/kaeso/blob/dev/openapi.json) describes every operation: its path, fields, answers and the one right it checks. The gateway generates it from its own code. To get the document of the version you run:

```bash
docker compose exec gateway kaeso openapi > openapi.json
```

## Authentication

| Method | Send | Acts with |
| --- | --- | --- |
| Platform key | `Authorization: Bearer sk-kso-org-…` | The Admin role's rights, in its organization and all its projects |
| API key with platform access | `Authorization: Bearer sk-kso-…` | Its member's or service account's rights, in its own project only |
| Session | The session cookie that the sign-in endpoints under `auth/*` set | Your own rights |

Use a platform key to automate a whole organization, and a project key for scripts and CI in one project. Sessions serve the console and people with curl. The session cookie is `__Host-kaeso_session`, or `kaeso_session` on an `http://` install at a loopback address such as localhost.

- **Keys reach only `orgs/{org}/*`.** `me/*`, `instance/*` and the signed-in routes of `auth/*` answer keys with 403. Organization-level routes answer project keys with 403.
- **Keys go in `Authorization`.** The Platform API never reads `x-api-key`.
- **Platform access is chosen when a key is created**, with `"access": "platform"` or `"both"`. The default is `inference`, and platform access needs the right `keys.platform_access.grant` ([API keys](../console/api-keys.md)).
- **Platform keys are created only in a session**, by someone who holds every right of the Admin role. They never call models.
- **Organization policies apply to sessions.** An organization that requires single sign-on answers other sessions with 403. One that requires two-factor authentication answers sessions without a second factor with 403, except those from its own single sign-on.
- **Unknown keys are throttled.** Too many requests with unknown keys from one address, on both APIs together, get `429 rate_limited` with `Retry-After` ([API key failures](../self-hosting/configuration.md#api-key-failures)).

### Cross-site requests

A request that carries the session cookie and changes something must send `Origin` with your install's origin, such as `Origin: https://kaeso.example.com`. The same holds for every such request to a public `auth/*` endpoint. Browsers send the header themselves, and `Referer` doesn't count in its place. Without it, or with another origin, Kaeso answers `403 permission_denied`, and the message names the address to open Kaeso at.

This keeps other sites from acting with your session, or from signing your browser in to an account of theirs.

- Requests with an API key or a platform key need no `Origin`.
- `GET`, `HEAD` and `OPTIONS` never need it, `GET auth/methods` and `GET auth/oidc/start` included.
- A server that calls `POST auth/methods`, `POST auth/password` or `POST auth/oidc/start` sends the install's origin. To send people from another site to an identity provider, link to `GET auth/oidc/start` instead ([sign-in and single sign-on](../self-hosting/sign-in.md#links-that-go-straight-to-the-provider)).

### Recent sign-in

In a session, some actions need a sign-in within the last 15 minutes. Otherwise Kaeso answers `403 recent_sign_in_required`, and the console asks you to sign in again: with your password, through single sign-on, or with a sign-in link, whichever your account and the install allow. Keys never need one.

`GET me/sign-in-again` lists how the account can confirm: `password`, `magic_link`, the organizations in `sso` whose identity provider it's linked to, and `set_password` when it has no password yet but can set one through `POST auth/password-reset`. Pass `org` for the organization of the action; when it requires single sign-on, only its own provider confirms. `POST me/sign-in-again/sso` starts a confirmation at one of them. The provider is asked for a fresh sign-in (`prompt=login`, `max_age=0`), only the same account confirms, and the session is replaced by one from that organization's single sign-on. On failure the browser comes back to `next` with `#sso_error=`, such as `other_account`, `link_required` or `stale_sign_in`. `no_auth_time` means the provider's answer didn't say when the person signed in; `sso` then leaves that organization out, and its connection shows `sends_auth_time: false` in `GET orgs/{org}/security`.

- **In organizations and projects:** the rights flagged `recent_sign_in` in `orgs/{org}/rights`. These are `org.delete`, `org.ownership.transfer`, `provider_credentials.create`, `provider_credentials.replace`, `security.sso.manage`, `security.sso.require`, `security.2fa.require`, `platform_keys.create`, `logs.delete` and `keys.platform_access.grant`.
- **In your account:** changing your email, setting a password when the account has none, turning off two-factor authentication, creating new recovery codes and deleting the account.
- **For instance admins:** changing account creation, organization creation or the allowed domains, choosing automatic updates, applying an update, and scheduling an organization for deletion or deleting it.

## Conventions

| Topic | Rule |
| --- | --- |
| Format | REST with JSON: paths in plural kebab case (`service-accounts`), fields in `snake_case`. Send bodies with `content-type: application/json`; Kaeso answers 400 to a field the endpoint doesn't know. |
| Ids | A prefix and a time-ordered part: `org_…`, `proj_…`, `usr_…`, `mem_…`, `inv_…`, `svc_…`, `key_…`, `opk_…`, `cred_…`, `model_…`, `alias_…`, `lim_…`, `role_…`, `ses_…`, `req_…`, `evt_…`, `exp_…`, `upd_…`. |
| Times | RFC 3339, in UTC. Every response names the install's time zone in `Kaeso-Time-Zone`, such as `Europe/Berlin`, the zone the console shows times in. |
| Money | US dollars as decimal strings, such as `"0.0001"`. Send prices and amounts as decimal strings or whole numbers; a JSON fraction such as `0.5` gets 400. |
| Limit amounts | At most 10^12 requests or tokens, or 1,000,000,000 USD for a spend budget. |
| Create | `POST` on the collection, answered with 201 and the new resource. |
| Update | `PATCH` with only the fields to change. |
| Actions | `POST …/archive`, `…/disable`, `…/enable`, `…/replace` or `…/restore`. Archiving keeps the record. |
| Delete | `DELETE`, only where something is really deleted: provider keys, aliases, catalogue models, limits, custom roles, sessions, invitations and your account. |
| Audit | Every change is written to the audit log of its level, in the same transaction. |
| Request ids | Every answer carries `kaeso-request-id`. |

### Lists

A list takes `limit`, from 1 to 100 with a default of 20, and `after`, the `next_cursor` of the previous page. Further query parameters filter the list. Lists come in creation order, except request logs and audit logs, which come newest first.

```json
{ "data": [], "has_more": true, "next_cursor": "…" }
```

`next_cursor` is `null` on the last page.

### Revisions

Catalogue models and limits carry a `revision` that goes up with every change. Reading, creating or changing one answers with `ETag: "<revision>"`. Send that tag back in `If-Match` with `PATCH` or `DELETE` to change only the revision you read. If the entry changed since, Kaeso answers `412 conflict`; without `If-Match`, the change applies to the current revision.

### Organizations scheduled for deletion

An organization scheduled for deletion is read-only until it is restored or deleted. Reads work as the caller's rights allow, and its keys don't authenticate at all. Every change in it answers `409 org_scheduled_for_deletion`, leaving it and accepting an invitation to it included. Only `POST orgs/{org}/restore` and the instance admins' `instance/orgs/{org}/*` still work.

`auth/session` marks such an organization in each membership with `org_state` and `org_deletion_due_at`. Its rights there shrink to seeing, exporting and `org.delete`. The session also carries the instance's `org_deletion` policy, with `allowed` and `grace_days`.

## Errors

Every error has the same body:

```json
{
  "error": {
    "type": "permission_denied",
    "message": "You need the right `models.manage` in this project; ask one of its admins for it.",
    "right": "models.manage"
  }
}
```

The message is one short sentence for people, and the console shows it as it is. `param` names the field, query or path parameter at fault. `right` names the right the caller lacks, when that one right would allow the request. Each is left out when it doesn't apply.

| Status | `type` | When |
| --- | --- | --- |
| 400 | `invalid_request` | The body isn't JSON, or has an unknown field. A parameter has the wrong type or form, or a value is out of range. |
| 401 | `authentication` | No session or key, or one that is unknown, expired or ended. |
| 403 | `permission_denied` | A missing right, a key on a route it can't use, a missing `Origin` or an organization policy. |
| 403 | `recent_sign_in_required` | The action needs a [recent sign-in](#recent-sign-in). |
| 404 | `not_found` | The path doesn't exist, or the thing doesn't exist or the caller can't see it. |
| 405 | `invalid_request` | The path doesn't take this method. `Allow` lists the ones it takes. |
| 409 | `conflict` | The change clashes with the current state, such as a name already taken. |
| 409 | `org_scheduled_for_deletion` | A change in an organization scheduled for deletion. |
| 412 | `conflict` | `If-Match` names an older revision. |
| 413 | `request_too_large` | The body is larger than `KAESO_BODY_LIMITS_PLATFORM_API`, 1 MiB by default. |
| 429 | `rate_limited` | Too many sign-in attempts or unknown keys from one address, or a limit on a Playground request. `Retry-After` says when to try again. |
| 500 | `internal` | A fault in the gateway. The message names the request id to report. |
| 503 | `unavailable` | Kaeso can't reach a database or another backing service, such as "Kaeso cannot reach its database right now; try again shortly." |
| 429, 502, 503, 504 | `provider_error` | The provider failed, only on [Playground](#playground) requests. |

## Routes

Routes are relative to `/platform/v1`. [`openapi.json`](#openapi-document) lists each one with its fields.

| Paths | Holds |
| --- | --- |
| `auth/*` | First-start setup, sign-in methods, password and two-factor sign-in, sign-in links, password reset, single sign-on, sessions, sign-up where the instance allows the address, and invitations. `auth/session` returns the user, their memberships with roles and rights, and whether they may create organizations (`can_create_orgs`). |
| `me`, `me/*` | Your profile, picture and language (`locale`: `auto`, `en`, `de` or `zh-CN`), email, password, two-factor authentication and recovery codes. Also your sessions, memberships, keys and notifications, creating an organization and deleting your account. |
| `orgs/{org}/*` | The organization and its picture, members, invitations, roles and rights, and projects. Also the providers Kaeso calls, provider keys, security settings, platform keys, log settings and storage, usage, the audit log, ownership transfer, deletion and restoring. |
| `orgs/{org}/projects/{project}/*` | The project and its picture, members with their model access, service accounts, keys with their models and limits, and key policy. Also the catalogue and aliases, limits, content logging, request logs and exports, usage, the audit log and the [Playground](#playground). |
| `instance/*` | For instance admins: users, organizations, instance settings, the models.dev sync, system health and storage, updates and the instance audit log. |

Instance admins see every organization's metadata, never its content. They create, rename, schedule for deletion, restore and delete organizations under `instance/orgs`. `POST instance/orgs/{org}/owner` makes any active user an owner, the admin included, which is how an instance admin gets in to manage an organization. The organization's audit log records it.

Only instance admins reset a user's two-factor authentication, with `POST instance/users/{user}/reset-2fa`. There are no billing routes: the rights `billing.view` and `billing.manage` are for Kaeso Cloud, flagged `cloud_only` in `orgs/{org}/rights`.

`GET orgs/{org}/providers` needs `org.view` and lists every provider Kaeso calls, the same in every organization. Each has its id and name, the page where you create its keys (`key_page`), how its keys start (`key_prefix`, `null` without a fixed prefix) and the formats Kaeso calls it with, the default first. [Supported providers](../concepts/supported-providers.md) shows the same list as a table.

## Roles and rights

Every organization member has one organization role. Every project member and service account has one project role. A role is built in or a custom role of the organization (`role_…`), and rights can be added to or removed from it per member. An organization role also holds rights in every project. [Rights](../concepts/rights.md) lists every right and built-in role.

| Route | Right | Answers with |
| --- | --- | --- |
| `GET orgs/{org}/rights` | `org.view` | Every right by category, with its name, description, flags (`sensitive`, `recent_sign_in`, `cloud_only`, `owner_only`) and the rights it needs. |
| `GET orgs/{org}/roles`, `GET …/roles/{role}` | `org.view` | Built-in and custom roles with their rights and number of holders. `?scope=org` or `?scope=project` filters them. |
| `POST orgs/{org}/roles`, `PATCH …/roles/{role}`, `DELETE …/roles/{role}` | `roles.manage` | Creates, changes or deletes a custom role. An organization has at most 50. |
| `GET orgs/{org}/members/{user}/rights` | `members.view` | A member's effective rights in the organization, in every project and in each project the caller can see, with their sources. |
| `GET orgs/{org}/projects/{project}/members/{user}/rights` | `project.members.view` | A project member's effective rights in this project, with their sources. |
| `GET orgs/{org}/projects/{project}/my-rights` | `project.view` | The caller's own rights in this project, with their sources. |
| `GET orgs/{org}/rights/{right}/grantors` | `members.view` | Up to 10 active members who hold the right and may change rights, owners first. |
| `GET orgs/{org}/projects/{project}/rights/{right}/grantors` | `project.members.view` | The same, for a right in this project. |

Send a grant as `{"role": "developer", "rights_added": [], "rights_removed": []}`. `role` is a built-in role's name or a custom role's id; `preset` is its deprecated name. The answer carries `role` as `{id, kind, name}`, the effective `rights` and, for organization grants, `project_rights`. Invitations take `role` and `project_roles: [{project_id, role}]`, and the single sign-on setup takes `default_role`.

Every change of rights follows the [grant rules](../concepts/rights.md#grant-rules):

- Giving a right you don't hold, or changing a member who holds a right you lack, gets 403, such as "You can't grant `audit_log.view`: you don't hold it yourself."
- Giving a right without one it needs, or removing one that another right needs, gets 400, and the message names both. An example is "`logs.delete` needs `logs.storage.view`; remove `logs.delete` too."

An effective right lists its sources:

| Source | The right comes from |
| --- | --- |
| `owner` | Being an owner of the organization |
| `role` | The organization role, for organization rights |
| `project_role` | The project role |
| `added` | A right added to the member |
| `org_role` | The organization role's rights in every project |
| `org_right` | An organization right that reaches into projects, such as `projects.view.all` |
| `key` | An API key acting in the project |

## Playground

The console's [Playground](../console/playground.md) sends requests to the project's models as the signed-in member. Both of its routes need a session and the project right `playground.use`. They answer keys with 403, since apps call models on `/v1`.

| Route | Does |
| --- | --- |
| `GET orgs/{org}/projects/{project}/playground/models` | Lists the models the member can call: the catalogue without embeddings models, narrowed by the member's model access. Each has its name, aliases, reasoning efforts, limits and prices, whether it takes `temperature` and whether the project has a provider key for it. |
| `POST orgs/{org}/projects/{project}/playground/responses` | Takes an OpenAI Responses request with `"stream": true` and answers with the event stream that `/v1/responses` sends, keepalive comments included. |

The request takes `model`, `input` (a string or input items), `instructions`, `temperature`, `top_p`, `max_output_tokens`, `reasoning.effort`, `reasoning.summary` and `stream`, which must be `true`. With a [session](automation.md#use-a-session-instead):

```bash
curl -N https://kaeso.example.com/platform/v1/orgs/org_…/projects/proj_…/playground/responses \
  -b "$JAR" \
  -H "Origin: https://kaeso.example.com" \
  -H "content-type: application/json" \
  -d '{"model": "claude-haiku-4-5", "input": "Write a haiku about gateways.", "stream": true}'
```

A Playground request is a model request like any other, made by the member without a key:

- **Same path.** The model resolves in the project's catalogue, aliases and `provider/model` included, and must be within the member's model access. Kaeso uses the project's provider key, and closing the stream cancels the provider's request.
- **Limits.** The member's and the project's rates and budgets apply. Key limits don't, as there is no key.
- **Usage and logs.** The request is charged to the project as the member's, and usage by `key` shows it as `playground`. Its log entry has the source `playground` and holds its content when the project logs content. `logs` and `logs/export` filter with `source=api` or `source=playground`.

Errors before the stream starts are Platform API errors:

| Status | `type` | Cause |
| --- | --- | --- |
| 400 | `invalid_request` | `param` is `model` for a model outside the catalogue. It is `stream` for `"stream": false`. |
| 404 | `not_found` | The provider doesn't serve the model to the project's provider key, with the message `/v1` apps get ([models the provider doesn't serve](errors.md#models-the-provider-doesnt-serve)), or the provider answered 404 for something else the request names. |
| 403 | `permission_denied` | The model is outside the member's model access. |
| 409 | `conflict` | The project is archived, or it has no provider key for the model's provider. |
| 409 | `org_scheduled_for_deletion` | The organization is scheduled for deletion. |
| 429 | `rate_limited` | A rate or budget is reached, and the message names it. |
| 503 | `unavailable` | Kaeso can't read the catalogue, the provider keys or the limits, or it is restarting. |
| 429, 502, 503, 504 | `provider_error` | The provider rate-limited the request, failed or refused the provider key, is overloaded, or timed out. |

Once the stream has started, errors arrive as Responses `error` events, as on `/v1`.
