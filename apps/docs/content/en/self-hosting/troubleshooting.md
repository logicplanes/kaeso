---
title: Troubleshooting
description: Common problems with an install and how to solve them.
order: 9
---

Start with the gateway's log; it names the setting, the request or the service involved:

```bash
docker compose logs --tail 100 gateway
```

## The gateway doesn't start

When a setting is missing or invalid, the gateway stops with a message that names it, `docker compose up -d --wait` fails on the gateway's container, and `docker compose ps` shows it restarting.

| The message says | Fix |
| --- | --- |
| A variable of `.env` is missing | Compose itself refuses to start and names it: set `KAESO_DOMAIN`, `POSTGRES_PASSWORD`, `POSTGRES_SUPERUSER_PASSWORD`, `CLICKHOUSE_PASSWORD` and `VALKEY_PASSWORD`, and check that `master-keys` exists next to `compose.yaml` |
| `KAESO_SMTP_HOST` is required | Add your SMTP server, or set `KAESO_MAIL_TRANSPORT=log` to try Kaeso without one; see [email](email.md) |
| Another setting is invalid | Correct it in `compose.override.yaml`; [configuration](configuration.md) lists the allowed values |
| A master key id isn't configured | The database holds secrets encrypted with a key missing from `master-keys`; add it back, see [master keys](backups.md#master-keys) |
| The database was migrated by a newer version of Kaeso | An older image is starting against a newer database; start the newer version, or [downgrade](upgrades.md#downgrade) |
| PostgreSQL or ClickHouse refuses the password | The password in `.env` changed after the databases were created, and they keep their first one; put the old password back |

## No setup link, or it expired

The setup link exists only while the install has no users, and works once, for 24 hours. Print a new one:

```bash
docker compose exec gateway kaeso setup-link
```

If Kaeso is already set up, the command says so; sign in at your domain instead.

## Locked out of an install without passwords

On an install that signs in only through single sign-on, a broken identity provider, or a setup session that ended before single sign-on worked, leaves no way in. Don't turn passwords back on: print a one-time sign-in link for an instance admin with `kaeso user sign-in-link`, as [getting back in](sign-in.md#getting-back-in) describes.

## The site has no certificate

Caddy needs the domain's DNS record to point at the server and ports 80 and 443 to be open. `docker compose logs caddy` shows the certificate authority's answer; see [certificates](domains-and-tls.md#certificates).

## Someone lost their second factor

An instance admin resets it under **Settings** › **Instance** › **Users**, in the person's sheet, with **Reset two-factor**. Without an instance admin who can sign in, reset it on the server:

```bash
docker compose exec gateway kaeso user reset-2fa person@example.com
```

Either way, the person's authenticator and recovery codes are removed, their sessions end, they get an email, and the instance audit log records it. Where two-factor authentication is required, they set it up again at their next sign-in.

## Requests fail

The error comes in the format the app called and says what to do; [errors](../api/errors.md) lists them all. The most common:

| Status | Why | What to do |
| --- | --- | --- |
| 400 | No provider key is set up for the model's provider | Add one under **Providers**, or assign one to the project |
| 401 | The key is missing, unknown, disabled, archived or expired | Check the key the app sends, and its state under **API keys** |
| 403 | The key, or its member, may not use the model | Widen the model access of the member or the key |
| 404 | The model isn't in the project's catalogue | Add it under **Models**, or call a model that is there |
| 429 | A rate limit or a budget, or too many unknown keys from one address | The message names the limit; wait for `Retry-After`, or raise the limit |
| 502, 504 | The provider couldn't be reached, didn't answer in time, or sent a response larger than `KAESO_BODY_LIMITS_PROVIDER_RESPONSE` | Check the provider's status; the request log shows its error |
| 503 | Valkey is down and a hard budget can't be checked, or the gateway is restarting | Check [`/readyz`](monitoring.md#health-checks) and `docker compose logs valkey` |

In the OpenAI formats, some of the gateway's own 503 errors arrive as 500.

## Streams stop after a minute

A proxy or load balancer between the app and Kaeso closes idle connections. The gateway writes a keepalive every 15 seconds while a stream is silent; if a proxy on the way closes connections sooner, lower `KAESO_STREAMS_KEEPALIVE_INTERVAL` in [`compose.override.yaml`](installation.md#local-changes). Behind your own proxy, also turn off response buffering for `/v1/`. See [long streams](domains-and-tls.md#long-streams).

## Request logs are missing or late

Entries appear a few seconds after the request. If none appear, ClickHouse may not be accepting writes: `/readyz` reports `degraded`, and `kaeso_log_buffer_rows` grows. Rows wait in memory up to `KAESO_LOGS_BUFFER_SIZE`, then the oldest are dropped. Check `docker compose logs clickhouse`, and the gateway's log for `ClickHouse migrations failed`.

## Someone reports a request id

When the gateway fails in a way the caller can't fix, the Platform API answers 500 and the console shows *Something went wrong in the gateway; report request req_… to your admin.* Every [log line](monitoring.md#logs) of a request carries its id, so `docker compose logs gateway | grep req_…` finds the `internal error` line with its cause.

## Kaeso cannot reach its database

When PostgreSQL is down or refuses connections, the console and the Platform API answer 503 with *Kaeso cannot reach its database right now; try again shortly.*, and [`/readyz`](monitoring.md#health-checks) answers `not_ready` with `postgres` `down`. Check `docker compose ps postgres` and `docker compose logs postgres`; a full disk and a changed password are the usual causes. Requests work again as soon as the database is back, without a restart.

The Platform API answers the same 503 when a request needs Valkey or ClickHouse and can't reach them, except request logs and usage, which say they are unavailable right now.

## Email doesn't arrive

Send a test with **Send test email** under **Settings** › **Instance** › **Settings**, and look in the gateway's log for the SMTP server's answer. Check `KAESO_SMTP_HOST`, the port, `KAESO_SMTP_TLS` and the credentials in `.env`, and that your SMTP server may send from the address in `KAESO_SMTP_FROM`. With `KAESO_MAIL_TRANSPORT=log`, emails are only logged. See [email](email.md).
