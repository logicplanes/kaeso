---
title: Troubleshooting
description: Common problems with an install and how to solve them.
order: 9
---

Start with the gateway's log. It names the setting, the request or the service involved.

```bash
docker compose logs --tail 100 gateway
```

## The gateway doesn't start

The gateway checks its settings at startup and stops with a message that names the setting. `docker compose up -d` then stops with an error about the gateway's container, and `docker compose ps` shows it restarting.

- Check that `.env` sets `KAESO_DOMAIN`, `POSTGRES_PASSWORD`, `POSTGRES_SUPERUSER_PASSWORD`, `CLICKHOUSE_PASSWORD` and `VALKEY_PASSWORD`, and that `master-keys` exists next to `compose.yaml`. Compose itself refuses to start without any of these variables and names the one that is missing.
- A message about `KAESO_SMTP_HOST` means email has nowhere to go: add your SMTP server, or set `KAESO_MAIL_TRANSPORT=log` to try Kaeso without one. See [email](email.md).
- Check that every value in `compose.override.yaml` is one the [configuration](configuration.md) allows.

If the message names a master key id that isn't configured, the database holds secrets encrypted with a key that is missing from `master-keys`. Add the key back; see [backups and master keys](backups.md#master-keys).

If it says the database was migrated by a newer version of Kaeso, an older image is starting against a newer database. Start the newer version, or restore the backup taken before the upgrade.

If PostgreSQL or ClickHouse refuse the gateway's password, the password in `.env` changed after the databases were created; the databases keep the password they started with. Put the old password back in `.env`.

## No setup link, or it expired

The link is printed only while the install has no users, and works once, for 24 hours. Print a new one:

```bash
docker compose exec gateway kaeso setup-link
```

If Kaeso is already set up, the command says so; sign in at your domain instead.

## The site has no certificate

Caddy needs the domain's DNS record to point at the server and ports 80 and 443 to be open. `docker compose logs caddy` shows the certificate authority's answer. See [domains and TLS](domains-and-tls.md#certificates).

## Someone lost their second factor

An instance admin resets it under **Settings** › **Instance** › **Users**, in the person's sheet, with **Reset two-factor**. Without an instance admin who can sign in, reset it from the server:

```bash
docker compose exec gateway kaeso user reset-2fa person@example.com
```

It removes the person's authenticator and recovery codes, ends their sessions, emails them, and records it in the instance's audit log. They set two-factor authentication up again at their next sign-in where it is required.

## Requests fail

The error comes in the format the app called, and says what to do. The [errors](../api/errors.md) page lists them. The most common:

| Status | Why | What to do |
| --- | --- | --- |
| 401 | The key is missing, unknown, disabled, archived or expired | Check the key the app sends, and its state under **API keys** |
| 403 | The key's member, or the key, may not use the model | Widen the model access of the member or the key |
| 404 | The model isn't in the project's catalogue | Add it under **Models**, or call a model that is there |
| 400 | No provider key is set up for the model's provider | Add one under **Providers**, or assign one to the project |
| 429 | A rate limit or a budget, or too many unknown keys from one address | The message names the limit; wait for `Retry-After`, or raise the limit |
| 502, 504 | The provider couldn't be reached, didn't answer in time, or sent a response larger than `KAESO_BODY_LIMITS_PROVIDER_RESPONSE` | Check the provider's status; the request log shows its error |
| 503 | Valkey is down and a hard budget can't be checked, or the gateway is restarting | Check [`/readyz`](monitoring.md#health-checks) and `docker compose logs valkey` |

In the OpenAI formats, some of the gateway's own 503 errors arrive as 500.

## Streams stop after a minute

A proxy or load balancer between the app and Kaeso closes idle connections. The gateway writes a keepalive every 15 seconds while a stream is silent; lower `KAESO_STREAMS_KEEPALIVE_INTERVAL` in [`compose.override.yaml`](installation.md#local-changes) if a proxy on the way closes connections sooner. If you run your own proxy, turn off response buffering for `/v1/`. See [long streams](domains-and-tls.md#long-streams).

## Request logs are missing or late

Entries appear a few seconds after the request. If none appear, ClickHouse may not be accepting writes: `/readyz` reports `degraded`, and the metric `kaeso_log_buffer_rows` grows. Rows wait in memory up to `KAESO_LOGS_BUFFER_SIZE`, then the oldest are dropped. Check `docker compose logs clickhouse`.

## Someone reports a request id

When the gateway fails in a way the caller can't fix, the Platform API answers 500 and the console shows *Something went wrong in the gateway; report request req_… to your admin.* Every [log line](monitoring.md#logs) a request causes carries its id, so `docker compose logs gateway | grep req_…` finds the line `internal error` with its cause.

## Kaeso cannot reach its database

When PostgreSQL is down or refuses connections, the console and the Platform API answer 503 with *Kaeso cannot reach its database right now; try again shortly.*, and [`/readyz`](monitoring.md#health-checks) answers `not_ready` with `postgres` `down`. Check `docker compose ps postgres` and `docker compose logs postgres`: a full disk and a changed password are the usual causes. Requests work again as soon as the database is back; nothing needs to restart. The Platform API answers the same 503 when Valkey or ClickHouse can't be reached and a request needs them, except request logs and usage, which say they are unavailable right now.

## Email doesn't arrive

Send a test from **Settings** › **Instance** › **Settings** › **Send test email**, and check the gateway's log for the SMTP server's answer. Check `KAESO_SMTP_HOST`, the port, `KAESO_SMTP_TLS` and the credentials in `.env`, and that your SMTP server may send from the address in `KAESO_SMTP_FROM`. If `KAESO_MAIL_TRANSPORT` is `log`, emails are only logged. See [email](email.md).
