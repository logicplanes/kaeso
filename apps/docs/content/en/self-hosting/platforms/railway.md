---
title: Railway
description: Deploy Kaeso on Railway from the infrastructure file, with Railway Postgres and Redis.
order: 2
---

`.railway/railway.ts` in the repository describes a Railway project in [Infrastructure as Code](https://docs.railway.com/infrastructure-as-code): a public edge, a private gateway and console, a Postgres service and a Redis service, all pinned to one release. Railway's older `railway.json` and `railway.toml` stop working on 2026-12-01, so the repository has none.

## What the file creates

| Service | Image | Reached by |
| --- | --- | --- |
| `kaeso-edge` | The console image, running `kaeso-edge` | Everyone, on the domain you give it |
| `kaeso-gateway` | The gateway image | The edge, on Railway's private network |
| `kaeso-console` | The console image | The edge, on Railway's private network |
| `kaeso-postgres` | Railway Postgres | The gateway |
| `kaeso-valkey` | Railway Redis | The gateway |

Railway's private network is IPv6 only, so the gateway listens on `[::]:8080` and trusts the edge from the unique local address range. Every image has automatic updates off, so the services stay on the release the file names.

## Deploy

1. Generate a master key, and keep it in a password manager:

   ```bash
   docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate
   ```

2. Install the Railway CLI, sign in and create an empty project:

   ```bash
   railway login
   railway init
   ```

3. In the project's **Settings**, under **Shared Variables**, add these:

   | Variable | What to enter |
   | --- | --- |
   | `KAESO_PUBLIC_URL` | `https://` and the edge's domain; set it to a placeholder now and correct it in step 6 |
   | `KAESO_MASTER_KEYS` | The key from step 1 |
   | `KAESO_SETUP_EMAIL` | Your address; the gateway emails the setup link there |
   | `KAESO_SMTP_HOST`, `KAESO_SMTP_PORT`, `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD`, `KAESO_SMTP_FROM` | Your SMTP server ([email](../email.md)) |
   | `KAESO_CLICKHOUSE_URL` | A ClickHouse HTTP URL with user and password for request logs, or empty to keep them off |

4. Put the release's `.railway/railway.ts` into a folder of your own, from `https://github.com/logicplanes/kaeso/blob/v{{version}}/.railway/railway.ts`, and apply it from that folder:

   ```bash
   railway config apply
   ```

5. Give `kaeso-edge` a domain: under its **Settings**, generate a Railway domain or add your own, for port 8080.
6. Set `KAESO_PUBLIC_URL` to `https://` and that domain, and redeploy `kaeso-gateway`. The gateway emails the one-time setup link to `KAESO_SETUP_EMAIL`; open it to create the first user ([the first start](../how-it-works.md#the-first-start)).

## Update and roll back

A release changes the image tags in `railway.ts`. Apply the new file the same way; Railway redeploys the services that changed, and the gateway migrates the databases at start, forward only ([upgrades](../upgrades.md)). To roll back, redeploy a service's earlier deployment from its **Deployments** list.

## What this is based on

The file follows Railway's TypeScript authoring reference, read on 2026-11-06: `defineRailway`, `project`, `service`, `image`, `postgres`, `redis`, service environments from `db.env` and `cache.env`, shared variables through `ctx.shared`, and `${{service.RAILWAY_PRIVATE_DOMAIN}}` for the private addresses. Railway marks the feature as experimental, so its API can still change. The three images were tested in this layout locally with Docker ([what was validated](index.md#what-was-validated)). The file has not yet been applied on Railway from an empty project; do that first, and check the private DNS names, the setup email, a streamed completion and an update.

## Next steps

- [Configuration](../configuration.md): Every setting the gateway reads, and what each one does.
- [Backups and master keys](../backups.md): Back up Postgres on Railway and keep the master key outside it.
- [Health and metrics](../monitoring.md): The health checks the edge and the gateway answer.
