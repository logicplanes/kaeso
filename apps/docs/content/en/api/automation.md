---
title: Automate with the Platform API
description: Set up a project, a model and a key for an app from a script, and read its usage and logs.
order: 4
---

Set up everything an app needs from a script: a project, a provider key, a model and the app's own API key. The requests use a platform key, which acts with the Admin role's rights in the whole organization.

## Before you begin

- A platform key, from **Platform keys** in the organization's settings, in the environment variable `KAESO_PLATFORM_KEY` ([platform keys](../console/api-keys.md#platform-keys)).
- The organization's id, `org_…`, from the console's address.
- An Anthropic API key in `ANTHROPIC_API_KEY`, unless the organization already has one.
- `curl` and `jq`.

Keep the organization's address in a variable, with your organization's id in place of `org_…`:

```bash
ORG_URL=https://kaeso.example.com/platform/v1/orgs/org_…
```

## Create a project

Create the project and keep its id:

```bash
PROJECT=$(curl -s "$ORG_URL/projects" \
  -H "Authorization: Bearer $KAESO_PLATFORM_KEY" \
  -H "content-type: application/json" \
  -d '{"name": "Support bot"}' | jq -r .id)
```

`PROJECT` now holds the new project's id, `proj_…`.

## Add a provider key

Add the organization's Anthropic key. Skip this step if the organization already has one.

```bash
curl -s "$ORG_URL/credentials" \
  -H "Authorization: Bearer $KAESO_PLATFORM_KEY" \
  -H "content-type: application/json" \
  -d "$(jq -n --arg secret "$ANTHROPIC_API_KEY" '{provider: "anthropic", name: "Production", secret: $secret}')"
```

The first key of a provider becomes its default, which every project uses unless you assign it another. The answer shows only the key's last four characters, and Kaeso never shows the secret again.

## Add a model

Add Claude Haiku to the project's catalogue:

```bash
curl -s "$ORG_URL/projects/$PROJECT/models" \
  -H "Authorization: Bearer $KAESO_PLATFORM_KEY" \
  -H "content-type: application/json" \
  -d '{"provider": "anthropic", "model_id": "claude-haiku-4-5"}'
```

Kaeso takes the model's prices, limits and capabilities from models.dev, and the answer shows them.

## Create the app's key

Give the app a service account with the Developer role, so its key doesn't depend on a person. Then create a key for it that expires in 90 days:

```bash
ACCOUNT=$(curl -s "$ORG_URL/projects/$PROJECT/service-accounts" \
  -H "Authorization: Bearer $KAESO_PLATFORM_KEY" \
  -H "content-type: application/json" \
  -d '{"name": "support-bot", "role": "developer"}' | jq -r .id)

KAESO_API_KEY=$(curl -s "$ORG_URL/projects/$PROJECT/service-accounts/$ACCOUNT/keys" \
  -H "Authorization: Bearer $KAESO_PLATFORM_KEY" \
  -H "content-type: application/json" \
  -d '{"name": "support-bot", "expires_in_days": 90}' | jq -r .secret)
```

`KAESO_API_KEY` now holds the app's key, `sk-kso-…`. Kaeso shows a key's secret only once, so store it where the app reads it.

## Call the model

Send a request with the new key, as the app will:

```bash
curl https://kaeso.example.com/v1/messages \
  -H "x-api-key: $KAESO_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{"model": "claude-haiku-4-5", "max_tokens": 1024, "messages": [{"role": "user", "content": "Write a haiku about gateways."}]}'
```

The answer is a Messages reply from Claude. The app's own code sets up its SDK as the [model API](model-api.md) shows.

## Read usage and logs

Read the project's usage by model, and its ten newest requests:

```bash
curl -s "$ORG_URL/projects/$PROJECT/usage?group_by=model" -H "Authorization: Bearer $KAESO_PLATFORM_KEY"
curl -s "$ORG_URL/projects/$PROJECT/logs?limit=10" -H "Authorization: Bearer $KAESO_PLATFORM_KEY"
```

A request shows in the logs within a few seconds. Usage includes costs when the caller has the right `costs.view`, and takes these query parameters:

- `group_by`: `day` (the default), `week`, `month`, `model`, `key` or `member`.
- `from` and `to`: days in UTC, such as `2026-08-15`. Grouped by day, they are at most 400 days apart.
- `usage/span` answers with the first and last day that have usage, so you can read a long period by week or month.

Add a log's id to read one request, such as `logs/req_…`. It includes the request's content when the project logs content.

## Use a project key instead

An API key with platform access works the same way inside its own project, with its member's or service account's rights. Send it as `Authorization: Bearer sk-kso-…`. It can add models with `models.manage` and read usage and logs with `usage.view` and `logs.metadata.view`.

Creating projects and adding provider keys are organization rights, which a project key never has. A key that belongs to a member can create more keys for that member with `POST orgs/{org}/projects/{project}/keys`. [API keys](../console/api-keys.md) explains how to give a key platform access.

## Use a session instead

Sign in with a password, keep the session cookie in a file, and send `Origin` with every request that changes something:

```bash
JAR=$(mktemp)

curl -s -c "$JAR" https://kaeso.example.com/platform/v1/auth/password \
  -H "Origin: https://kaeso.example.com" \
  -H "content-type: application/json" \
  -d '{"email": "owner@example.com", "password": "correct horse battery staple"}'
```

The requests above then work with `-b "$JAR" -H "Origin: https://kaeso.example.com"` in place of the `Authorization` header. A session acts with your own rights, and creates keys for your own user with `POST orgs/{org}/projects/{project}/keys`.

With two-factor authentication on, `auth/password` answers 202 with a `challenge`. Send it to `POST auth/totp` with `{"challenge": "…", "code": "123456"}`, or with `recovery_code` in place of `code`. Password sign-in works only while it is on for the install, which is the default. On a new install, the first user signs up through the setup link from `kaeso setup-link` ([installation](../self-hosting/installation.md)).

## Next steps

- [Platform API](platform-api.md): Look up every route, the conventions, errors and rights.
- [API keys](../console/api-keys.md): Limit a key to the models and rates an app needs.
- [Limits and budgets](../console/limits-and-budgets.md): Cap a project's requests, tokens and spend.
