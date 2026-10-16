---
title: Platform API
description: Manage an install over HTTP, as the console does.
order: 3
---

The Platform API manages organizations, projects, members, keys, provider keys, models, limits, usage and logs. It lives under `/platform/v1` on the same domain as the model API. The console uses it and nothing else, so everything the console does, a script can do too.

The complete reference is the OpenAPI document, [`openapi.json`](https://github.com/logicplanes/kaeso/blob/dev/openapi.json) at the root of the repository, generated from the gateway's code; `kaeso openapi` prints the document of the version you run, for example `docker compose exec gateway kaeso openapi`. Every operation in it names the one right it checks.

## Conventions

| Topic | Rule |
| --- | --- |
| Shape | REST with JSON; resource names in plural kebab case (`service-accounts`), fields in `snake_case`; bodies are sent with `content-type: application/json`, and a field the endpoint doesn't know is refused |
| Times | RFC 3339, in UTC |
| Money | US dollars as decimal strings, such as `"0.0001"` |
| Ids | A prefix and a time-ordered part: `org_…`, `proj_…`, `usr_…`, `key_…`, `opk_…`, `svc_…`, `cred_…`, `model_…`, `alias_…`, `lim_…`, `role_…`, `req_…`, `evt_…` and others |
| Lists | `limit` (1 to 100, default 20) and `after`, the `next_cursor` of the previous page; the answer is `{"data": [...], "has_more": true, "next_cursor": "…"}`, with `next_cursor` `null` on the last page; ordered by id, which is creation time, and request logs and audit logs newest first; filters are query parameters |
| Create | `POST` on the collection, answered with `201` and the resource |
| Update | `PATCH` with the fields to change |
| Actions | `POST …/archive`, `…/disable`, `…/enable`, `…/replace`, `…/restore`; archiving is not deleting |
| Delete | `DELETE`, only where something is really deleted, such as provider keys, aliases, catalogue models, limits, custom roles, sessions, invitations, the account |
| Rights | Each operation on `orgs/{org}/*` checks one right on the organization or project in its path, named in its description; `auth/*`, `me/*` and `instance/*` follow their own rules: public, the signed-in user, instance admins |
| Recent sign-in | With a session, sensitive actions need a sign-in in the last 15 minutes: deleting or restoring an organization, platform access for keys, adding and changing provider keys, creating platform keys, single sign-on and two-factor settings, deleting logs and others; otherwise the answer is `403 recent_sign_in_required` |
| Organizations scheduled for deletion | Read-only until restored or deleted: reads work as the caller's rights allow, and every change in the organization, leaving it and accepting an invitation to it included, is `409 org_scheduled_for_deletion`, except `POST orgs/{org}/restore` and the instance admins' `instance/orgs/{org}/*`; its keys don't authenticate at all |
| Audit | Every change is written to the audit log of its level in the same transaction |
| Request ids | Every answer carries `kaeso-request-id` |

`auth/session` shows a scheduled organization in each membership: its `org_state`, `org_deletion_due_at`, and rights reduced to seeing, exporting and `org.delete`. It also carries the instance's `org_deletion` policy (`allowed`, `grace_days`).

## Authentication

| Method | How | For |
| --- | --- | --- |
| Session | The HttpOnly `__Host-kaeso_session` cookie, `kaeso_session` on an `http://` install at a loopback address such as localhost, set by the sign-in endpoints under `auth/*`; changes must also send `Origin` with the origin of the install's public URL | The console, and people with curl |
| API key with platform access | `Authorization: Bearer sk-kso-…`; acts with its member's or service account's rights, only in its own project | Scripts and CI for one project |
| Platform key | `Authorization: Bearer sk-kso-org-…`; acts with the Admin role's rights in its organization and every project in it, and never calls models | Automation for a whole organization |

A project key has inference access by default. Platform access is chosen when the key is created, with `"access": "platform"` or `"both"`, and needs the right `keys.platform_access.grant`. Platform keys are created only with a session. See [API keys](../console/api-keys.md).

Keys are for `orgs/{org}/*`: `me/*`, `instance/*` and the signed-in routes of `auth/*` refuse them with `403`, and so do organization routes for a project key. An organization that requires single sign-on refuses other sessions with `403`, and one that requires two-factor authentication refuses password sessions without a second factor.

Too many requests with unknown API keys from one address, on the model API and the Platform API together, are refused with `429 rate_limited` and `Retry-After` until the window ends. [API key failures](../self-hosting/configuration.md#api-key-failures) has the settings.

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

| Status | `type` |
| --- | --- |
| 400 | `invalid_request` |
| 401 | `authentication` |
| 403 | `permission_denied`, or `recent_sign_in_required` |
| 404 | `not_found` |
| 405 | `invalid_request`, with `Allow` |
| 409 | `conflict`, or `org_scheduled_for_deletion` for a change in an organization waiting to be deleted |
| 413 | `request_too_large`, for a body larger than `KAESO_BODY_LIMITS_PLATFORM_API`, 1 MiB by default |
| 429 | `rate_limited`, with `Retry-After` |
| 500 | `internal`; the message names the request id |
| 502, 503, 504, 429 | `provider_error`, only for the Playground's model requests |
| 503 | `unavailable` |

`param` names the field, query or path parameter at fault, and `right` the right the caller lacks when one right would allow the request; both are left out when they don't apply. A parameter of the wrong type or form, or a body that isn't JSON, is `400 invalid_request`, and an unknown path `404 not_found`. The message is one short sentence for people; the console shows it as it is.

## A walk through with curl

These requests sign in with a password, then use the session cookie. They need `jq`.

```bash
KAESO=https://kaeso.example.com
JAR=$(mktemp)
```

Sign in. The first user of an install signs up through the setup link instead.

```bash
curl -s -c "$JAR" -H "Origin: $KAESO" -H 'content-type: application/json' \
  -d '{"email": "owner@example.com", "password": "correct horse battery"}' \
  "$KAESO/platform/v1/auth/password"
```

With two-factor authentication on, the answer is `202` with a `challenge`. Finish with `POST /platform/v1/auth/totp` and `{"challenge": "…", "code": "123456"}`, or a `recovery_code` instead of the code.

Take the organization from the session, and create a project:

```bash
ORG=$(curl -s -b "$JAR" "$KAESO/platform/v1/auth/session" | jq -r '.memberships[0].org_id')

PROJECT=$(curl -s -b "$JAR" -H "Origin: $KAESO" -H 'content-type: application/json' \
  -d '{"name": "Support bot"}' \
  "$KAESO/platform/v1/orgs/$ORG/projects" | jq -r .id)
```

Add the organization's Anthropic key. The secret is never shown again; only its last four characters stay visible. The first key of a provider is its default, used by every project without a key assigned.

```bash
curl -s -b "$JAR" -H "Origin: $KAESO" -H 'content-type: application/json' \
  -d '{"provider": "anthropic", "name": "Production", "secret": "sk-ant-…"}' \
  "$KAESO/platform/v1/orgs/$ORG/credentials"
```

Add a model to the project's catalogue. Its values come from models.dev.

```bash
curl -s -b "$JAR" -H "Origin: $KAESO" -H 'content-type: application/json' \
  -d '{"provider": "anthropic", "model_id": "claude-haiku-4-5"}' \
  "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/models"
```

Create an API key for the app. The answer's `secret` is the key, shown once.

```bash
KEY=$(curl -s -b "$JAR" -H "Origin: $KAESO" -H 'content-type: application/json' \
  -d '{"name": "support-bot", "expires_in_days": 90}' \
  "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/keys" | jq -r .secret)
```

Call the model API with an official SDK, or with curl:

```bash
curl -s "$KAESO/v1/messages" -H "x-api-key: $KEY" -H 'anthropic-version: 2023-06-01' \
  -H 'content-type: application/json' \
  -d '{"model": "claude-haiku-4-5", "max_tokens": 100, "messages": [{"role": "user", "content": "Hello"}]}'
```

Read usage and costs, by day (the default), `week`, `month`, `model`, `key` or `member`. `usage/span` gives the first and last day with usage, `{"first_day": "2026-08-15", "last_day": "…"}`, to read a long period by week or month. By day, `from` and `to` are at most 400 days apart.

```bash
curl -s -b "$JAR" "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/usage?group_by=model"
curl -s -b "$JAR" "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/usage/span"
curl -s -b "$JAR" "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/usage?from=2026-08-15&group_by=week"
```

Read the request log, newest first, and one entry with its content. The content needs the right `logs.content.view`.

```bash
curl -s -b "$JAR" "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/logs?limit=10"
curl -s -b "$JAR" "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/logs/req_…"
```

The project's calls work with a key that has platform access too: replace the cookie and `Origin` with `-H "Authorization: Bearer $KAESO_PLATFORM_KEY"`. Each needs its right: `models.manage`, `keys.own.create`, `usage.view` and `logs.metadata.view`. Adding a provider key and creating a project are organization rights, which a project key never has; use a platform key or a session for them.

## Areas

| Paths | What |
| --- | --- |
| `auth/*` | First-start setup, sign-in methods, password and two-factor, sign-in links, password reset, single sign-on, sessions, sign-up where the instance allows the address, invitations |
| `me`, `me/*` | Your profile, picture and language (`locale`: `auto`, `en`, `de`, `zh-CN`), email, password, two-factor and recovery codes, sessions, memberships, your keys, notifications, creating an organization, deleting the account |
| `orgs/{org}/*` | The organization and its picture, members, invitations, roles and rights, projects and the default one, provider keys (several per provider, a default, assignments to projects), security (single sign-on, required single sign-on and two-factor, key lifetime), platform keys, log settings, storage and deletions, usage, the audit log, ownership transfer, deletion and restoring |
| `orgs/{org}/projects/{project}/*` | The project and its picture, members and their model access, service accounts, keys with their models and limits, key policy, the catalogue (search, preview, batch) and aliases, limits, content logging, request logs and exports, usage, the audit log, the [Playground](#playground) |
| `instance/*` | Users, their invitations, sessions and instance admin flag, organizations with their members, projects, usage and storage, settings with a test email, the models.dev status and sync, system health, storage and log buffer, the instance audit log; for instance admins |

Instance admins see every organization's metadata, never its content. They create, rename, schedule for deletion, restore and delete organizations under `instance/orgs`, and make any active user an owner with `POST instance/orgs/{org}/owner`, themselves included; that is their way in to manage an organization, and it is recorded in its audit log. Only they reset a user's two-factor authentication, with `POST instance/users/{user}/reset-2fa`.

There are no billing routes. The rights `billing.view` and `billing.manage` are for Kaeso Cloud and carry the flag `cloud_only` in `orgs/{org}/rights`.

## Roles and rights

Every organization member has one organization role, and every project member and service account one project role, each with rights added or removed individually. Roles are built in or custom roles of the organization (`role_…`). An organization role also holds rights in every project. [Rights](../concepts/rights.md) lists every right and built-in role.

| Route | Right | What |
| --- | --- | --- |
| `GET orgs/{org}/rights` | `org.view` | Every right by category, with its name, description, flags (`sensitive`, `recent_sign_in`, `cloud_only`) and the rights it needs |
| `GET orgs/{org}/roles`, `GET …/roles/{role}` | `org.view` | Built-in and custom roles with their rights, rights in every project and number of holders; `?scope=org` or `project` |
| `POST orgs/{org}/roles`, `PATCH …/roles/{role}`, `DELETE …/roles/{role}` | `roles.manage` | Create, change and delete custom roles, at most 50; a change applies to every holder at once, and a role somebody holds can't be deleted |
| `GET orgs/{org}/members/{user}/rights` | `members.view` | A member's effective organization rights, rights in every project, and rights in each of their projects the caller can see, each with its sources |
| `GET orgs/{org}/projects/{project}/members/{user}/rights` | `project.members.view` | A member's effective rights in this project, with sources |
| `GET orgs/{org}/projects/{project}/my-rights` | `project.view` | The caller's own rights in this project, with sources |
| `GET orgs/{org}/rights/{right}/grantors`, `GET …/projects/{project}/rights/{right}/grantors` | `members.view`, `project.view` | Up to 10 active members who hold the right and may change rights, owners first: whom to ask |
| `GET orgs/{org}/projects/{project}/audit-log` | `project.audit_log.view` | The changes made in the project |

Grants are sent as `{"role": "developer", "rights_added": [], "rights_removed": []}`, where `role` is a built-in role's name or a custom role's id; `preset` is its deprecated name. They come back with `role` (`{id, kind, name}`), the effective `rights` and, for organization grants, `project_rights`. Invitations take `role` and `project_roles: [{project_id, role}]`; the single sign-on setup takes `default_role`.

Every change of rights follows the grant rules:

- It gives only rights the caller holds: organization rights in the organization, project rights in that project, rights in every project only if the caller holds them in every project.
- It changes or removes only members whose rights the caller all holds.
- Only owners make, change, remove or invite owners.
- Every right it gives comes with the rights it needs, or the answer is `400`, such as "`logs.delete` needs `logs.storage.view`."

The sources of an effective right are `owner`, `role` (the organization role, for organization rights), `project_role`, `added`, `org_role` (the organization role's rights in every project), `org_right` (an organization right that reaches into projects, such as `projects.view.all`) and `key` (an API key acting in the project).

## Playground

The console's [Playground](../console/playground.md) sends requests to the project's models as the signed-in member. Both routes need the project right `playground.use` and a session; API keys and platform keys are refused with `403`, since apps call models on `/v1`.

| Route | What |
| --- | --- |
| `POST orgs/{org}/projects/{project}/playground/responses` | An OpenAI Responses request with `"stream": true` (`model`, `input` as a string or input items, `instructions`, `temperature`, `top_p`, `max_output_tokens`, `reasoning.effort`, `reasoning.summary`); the answer is the Responses event stream `/v1/responses` sends, with keepalive comments |
| `GET orgs/{org}/projects/{project}/playground/models` | The models the member can call there: the catalogue without embeddings models, narrowed by the member's model access, with the name to send, aliases, whether it reasons, its efforts, whether it takes `temperature`, limits, prices and whether the project has a provider key for it |

A Playground request is a model request like any other, made by the member without a key:

- **Same path.** The model is resolved in the project's catalogue, aliases and `provider/model` included, and checked against the member's model access. The project's provider key is used, and closing the stream cancels the provider request.
- **Limits.** The member's and the project's rates and budgets apply; key limits don't, as there is no key.
- **Usage and logs.** The request is charged to the project, counted as the member's (usage by `key` shows it as `playground`), and logged with the source `playground`, with its content when the project logs content. `logs` and `logs/export` filter by `source=api` or `source=playground`.
- **Errors.** Before the stream starts, failures are Platform API errors: `400 invalid_request` (`param` is `model` for a model outside the catalogue, `stream` for `"stream": false`), `403 permission_denied` for a model outside the member's model access, `409 conflict` for an archived project or a provider without a key, `409 org_scheduled_for_deletion`, `429 rate_limited` naming the limit, `503 unavailable`, and `provider_error` when the provider fails or refuses the provider key (`502`), rate limits it (`429`), is overloaded (`503`) or times out (`504`). After the stream started, errors are Responses `error` events, as on `/v1`.

```bash
curl -N -b "$JAR" -H "Origin: $KAESO" -H 'content-type: application/json' \
  -d '{"model": "claude-haiku-4-5", "input": "Hello", "stream": true, "reasoning": {"summary": "auto"}}' \
  "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/playground/responses"
```

## How the console uses it

The console is served on the same origin as the gateway, so the session cookie is first-party and there is no CORS. It:

- signs people in through `auth/*` and keeps only the HttpOnly cookie;
- sends `Origin` with every change, as browsers do by themselves;
- reads `auth/session` for the signed-in user, their memberships with their organization role, rights and rights in every project, whether they are an instance admin and whether they may create organizations (`can_create_orgs`);
- reads a project's `my-rights` to decide what to show there, and takes right names, groups and dependencies from `orgs/{org}/rights`;
- pages lists with the cursors above, and fills the model forms from `models/search` and `models/preview`;
- shows `recent_sign_in_required` as a prompt to sign in again, then repeats the action;
- downloads exports (`logs/export`, `audit-log/export`) as streamed files;
- streams Playground replies with `fetch`; **Stop** aborts the fetch, which cancels the provider request;
- charts usage by day up to about three months, by week up to about a year and by month beyond, and starts **All time** at the `first_day` of `usage/span`.
