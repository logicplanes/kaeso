---
title: Railway
description: Deploy Kaeso on Railway with the Deploy on Railway button or the infrastructure file, with Railway Postgres and Redis.
order: 2
---

The **Deploy on Railway** button deploys Kaeso's [Railway template](https://railway.com/deploy/cmVV60). `.railway/railway.ts` in the repository describes the same project in [Infrastructure as Code](https://docs.railway.com/infrastructure-as-code), for the Railway CLI. Both run a public edge, a private gateway and console, a Postgres service and a Redis service, all pinned to one release. Railway's older `railway.json` and `railway.toml` stop working on 2026-12-01, so the repository has none.

## What Railway runs

| Service | Image | Reached by |
| --- | --- | --- |
| `kaeso-edge` | The console image, running `kaeso-edge` | Everyone, on its Railway domain or your own |
| `kaeso-gateway` | The gateway image | The edge, on Railway's private network |
| `kaeso-console` | The console image | The edge, on Railway's private network |
| `kaeso-postgres` | Railway Postgres | The gateway |
| `kaeso-valkey` | Railway Redis | The gateway |

Railway's private network is IPv6 only, so the gateway listens on `[::]:8080` and trusts the edge from the unique local address range. Railway's health checks call the port in `PORT`, so each service sets it to the port it listens on. Every image has automatic updates off, so the services stay on the release they were deployed with.

## Deploy with the button

1. Open [Deploy on Railway](https://railway.com/new/template/cmVV60?utm_medium=integration&utm_source=button&utm_campaign=kaeso) and sign in to Railway.
2. Under `kaeso-gateway`, fill in these variables:

   | Variable | What to enter |
   | --- | --- |
   | `KAESO_SETUP_EMAIL` | Your address; the gateway emails the setup link there |
   | `KAESO_SMTP_HOST`, `KAESO_SMTP_FROM` | Your SMTP server and the sender of Kaeso's emails ([email](../email.md)); the gateway doesn't start without an SMTP server |
   | `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD` | The server's user name and password, if it needs them |
   | `KAESO_SMTP_PORT` | `587` unless your server uses another port |
   | `KAESO_CLICKHOUSE_URL` | A ClickHouse HTTP URL with user and password for request logs, or empty to keep them off |

   The template fills in the rest: `KAESO_MASTER_KEYS` with a new master key, 32 random bytes from Railway's `secret()` function, and `KAESO_PUBLIC_URL` with the edge's Railway domain.

3. Deploy, and wait until the five services are online. The gateway emails the one-time setup link to `KAESO_SETUP_EMAIL`; open it to create the first user ([the first start](../how-it-works.md#the-first-start)).
4. Copy `KAESO_MASTER_KEYS` from the variables of `kaeso-gateway` into a password manager. Kaeso encrypts provider keys with it, so a database backup can't be read without it ([backups and master keys](../backups.md)).
5. For your own domain, add it to `kaeso-edge` under **Settings**, set `KAESO_PUBLIC_URL` on `kaeso-gateway` to `https://` and that domain, and redeploy the gateway.

> [!NOTE]
> The gateway image has no shell, so `railway ssh` can't run `kaeso setup-link` in it. If the setup email doesn't arrive, the gateway's logs name the SMTP error. Correct the SMTP variables and redeploy `kaeso-gateway`: it sends a new link at every start while no user exists.

## Deploy with the CLI

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
   | `KAESO_SMTP_HOST`, `KAESO_SMTP_PORT`, `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD`, `KAESO_SMTP_FROM` | Your SMTP server ([email](../email.md)); the gateway doesn't start without one |
   | `KAESO_CLICKHOUSE_URL` | A ClickHouse HTTP URL with user and password for request logs, or empty to keep them off |

4. Put the release's `.railway/railway.ts` into a folder of your own, from `https://github.com/logicplanes/kaeso/blob/v{{version}}/.railway/railway.ts`, install Railway's TypeScript package there and apply the file:

   ```bash
   npm install railway
   railway config apply
   ```

5. Give `kaeso-edge` a domain: under its **Settings**, generate a Railway domain or add your own, for port 8080.
6. Set `KAESO_PUBLIC_URL` to `https://` and that domain, and redeploy `kaeso-gateway`. The gateway emails the one-time setup link to `KAESO_SETUP_EMAIL`; open it to create the first user ([the first start](../how-it-works.md#the-first-start)).

## Update and roll back

A release changes the image tags. For a project from the button, change the tag in the image of `kaeso-gateway`, `kaeso-console` and `kaeso-edge` under each service's **Settings**, gateway first. For a project from the file, apply the new release's `railway.ts` the same way as before; Railway redeploys the services that changed. Either way, the gateway migrates the databases at start, forward only ([upgrades](../upgrades.md)). To roll back, redeploy a service's earlier deployment from its **Deployments** list.

## What this is based on

The file follows Railway's TypeScript authoring reference, read on 2026-11-06: `defineRailway`, `project`, `service`, `image`, `postgres`, `redis`, service environments from `db.env` and `cache.env`, shared variables through `ctx.shared`, and `${{service.RAILWAY_PRIVATE_DOMAIN}}` for the private addresses. Railway marks the feature as experimental, so its API can still change.

On 2026-11-09, the file was applied to an empty Railway project, and the template was deployed into another one, both with the eighth release candidate. In both, the five services came online, the edge served the console and the APIs on the Railway domain, and the first user was set up and signed in. The first run found that the gateway and the console failed Railway's health checks without `PORT`; the file and the template now set it. The test used a placeholder SMTP server, so the setup link came from `kaeso setup-link` through an SSH tunnel instead of email. Email delivery, a streaming reply and an update weren't tested on Railway.

## Next steps

- [Configuration](../configuration.md): Every setting the gateway reads, and what each one does.
- [Backups and master keys](../backups.md): Back up Postgres on Railway and keep the master key outside it.
- [Health and metrics](../monitoring.md): The health checks the edge and the gateway answer.
