---
title: Troubleshooting
description: Find the message or symptom you see on an install, and fix it.
order: 11
---

Find the message or the symptom you see, then follow its fix. Most problems show in the gateway's log, which names the setting, the request or the service involved:

```bash
docker compose logs --tail 100 gateway
```

## The gateway keeps restarting

The gateway stops when a setting is missing or invalid, and its container restarts. `docker compose up -d --wait` then fails on the gateway.

1. Run `docker compose logs --tail 20 gateway` and find the line that starts with `configuration:`. It names the setting, never its value.
2. Correct the setting in `compose.override.yaml`, or in `.env` if it is one of the [settings in .env](configuration.md#settings-in-env).
3. Run `docker compose up -d --wait` again.

## Compose says to set a variable in .env

A required variable of `.env` is missing, so Compose refuses to start and names it, such as `set KAESO_DOMAIN in .env`.

1. Set `KAESO_DOMAIN`, `POSTGRES_PASSWORD`, `POSTGRES_SUPERUSER_PASSWORD`, `CLICKHOUSE_PASSWORD` and `VALKEY_PASSWORD` in `.env`.
2. Check that `master-keys` exists next to `compose.yaml`.
3. Run `docker compose up -d --wait` again.

## KAESO_SMTP_HOST is required

The gateway has no SMTP server to send email with.

1. Add your SMTP server to `.env`, as [set up SMTP](email.md#set-up-smtp) shows.
2. Or, to try Kaeso without one, set `KAESO_MAIL_TRANSPORT=log` in `.env`.
3. Run `docker compose up -d --wait`.

## Stored secrets are encrypted with master keys that are not configured

The database holds secrets encrypted with a master key that is missing from `master-keys`. The message names the key id.

1. Find the key with that id in your copy of the master keys.
2. Add it back to `master-keys`, after the active key ([rotate the master key](backups.md#rotate-the-master-key)).
3. Recreate the gateway with `docker compose up -d --wait --force-recreate gateway`.

## The database was migrated by a newer version of Kaeso

An older gateway is starting against a database that a newer release migrated, and migrations can't be undone.

1. Start the newer release's `compose.yaml` again.
2. To go back to the older release, restore a backup from before the upgrade ([downgrade](upgrades.md#downgrade)).

## PostgreSQL refuses the gateway's password

`POSTGRES_PASSWORD` in `.env` changed after PostgreSQL's volume was created. PostgreSQL keeps the password it had then.

1. Put the old password back in `.env`.
2. Run `docker compose up -d --wait`.

## The site has no certificate

Caddy couldn't get a certificate for `KAESO_DOMAIN`, usually because of DNS or a firewall.

1. Run `docker compose logs caddy` and read the certificate authority's answer.
2. Check that the domain's DNS record points at the server, and that ports 80 and 443 are open to the internet.
3. Run `docker compose restart caddy`.

## The setup link is missing or expired

The setup link works once, for 24 hours, and only while the install has no users. Print a new one:

```bash
docker compose exec gateway kaeso setup-link
```

If Kaeso is already set up, the command says so. The setup page says the same, with a link to sign in. Sign in at your domain instead.

## Email doesn't arrive

The gateway can't reach the SMTP server, or the server refuses the email.

1. Choose **Send test email** under **Instance** › **System**.
2. Read the SMTP server's answer in the gateway's log.
3. Check `KAESO_SMTP_HOST`, `KAESO_SMTP_PORT`, `KAESO_SMTP_TLS` and the credentials in `.env`, and that the server may send from `KAESO_SMTP_FROM`.

With `KAESO_MAIL_TRANSPORT=log`, the gateway only logs emails and never sends them ([email](email.md)).

## Requests fail

The error comes in the format the app called, and its message says what to do. [Errors](../api/errors.md) lists every error; these are the most common:

| Status | Cause | Fix |
| --- | --- | --- |
| 400 | No provider key is set up for the model's provider | Add one under **Providers**, or assign one to the project |
| 401 | The key is missing, unknown, disabled, archived or expired | Check the key the app sends, and its state under **API keys** |
| 404 | The model isn't in the project's catalogue, or the key or its member may not use it | Add it under **Models**, or call a model that is there; if it is there, check the model access of the key and of its member |
| 429 | A rate limit or a budget, or too many unknown keys from one address | Wait for `Retry-After`, or raise the limit the message names |
| 502, 504 | The provider couldn't be reached, didn't answer in time, or sent a response larger than `KAESO_BODY_LIMITS_PROVIDER_RESPONSE` | Check the provider's status; the request log shows its error |
| 503 | Valkey is down and a hard budget can't be checked, or the gateway is restarting | Check [`/readyz`](monitoring.md#health-checks) and `docker compose logs valkey` |

In the OpenAI formats, some of the gateway's own 503 errors arrive as 500.

## Streams stop after a minute

A proxy or load balancer between the app and Kaeso closes connections that stay idle too long.

1. Lower `KAESO_STREAMS_KEEPALIVE_INTERVAL` in `compose.override.yaml` below the proxy's idle timeout. The default is 15 seconds.
2. Behind your own proxy, turn off response buffering for `/v1/`.

[Long streams](domains-and-tls.md#long-streams) explains the keepalives.

## Request logs are missing or late

Entries appear a few seconds after the request. When none appear, ClickHouse isn't accepting writes: `/readyz` reports `degraded`, and `kaeso_log_buffer_rows` grows. When the console shows no **Request logs** at all and `/readyz` reports `clickhouse` as `off`, the install keeps no request logs ([choose how to keep request logs](installation.md#choose-how-to-keep-request-logs)).

1. Run `docker compose logs clickhouse` and fix what it reports.
2. Search the gateway's log for `ClickHouse migrations failed`.

Rows wait in the gateway's memory up to `KAESO_LOGS_BUFFER_SIZE`, then Kaeso drops the oldest.

## Kaeso cannot reach its database

The console and the Platform API answer 503 with "Kaeso cannot reach its database right now; try again shortly." PostgreSQL is down or refuses connections, and [`/readyz`](monitoring.md#health-checks) answers `not_ready`.

1. Run `docker compose ps postgres` and `docker compose logs postgres`.
2. Fix the cause. A full disk and a changed password are the usual ones.

Requests work again as soon as the database is back, without a restart. The Platform API answers the same 503 when a request needs Valkey or ClickHouse and can't reach them.

## Someone lost their second factor

An instance admin resets it under **Instance** › **Users**, in the person's sheet, with **Reset two-factor**. Without an instance admin who can sign in, reset it on the server:

```bash
docker compose exec gateway kaeso user reset-2fa owner@example.com
```

Either way, Kaeso removes their authenticator and recovery codes, ends their sessions and sends them an email. Where two-factor authentication is required, they set it up again at their next sign-in.

## Two accounts share one address

Kaeso treats email addresses that differ only in how they are written as one address. That covers upper and lower case, an accent typed as part of its letter or after it, and full-width letters. Accounts created before Kaeso compared addresses this way can share one.

Kaeso then signs neither in by that address, sends neither a link, and logs `accounts share one canonical email address; refusing to look either up by it` with both user ids. An instance admin finds both under **Instance** › **Users** and deletes the one that shouldn't exist. The other then signs in as before.

## Nobody can sign in through single sign-on

On an install without passwords, a broken identity provider leaves no way in. Don't turn passwords back on: print a one-time sign-in link for an instance admin, as [get back in](sign-in.md#get-back-in) shows.

## Someone reports a request id

The gateway failed in a way the caller can't fix, and its log has the cause. The console showed "Something went wrong in the gateway; report request req_… to your admin."

1. Search the log for the id: `docker compose logs gateway | grep req_…`.
2. Read the `internal error` line with its cause.
