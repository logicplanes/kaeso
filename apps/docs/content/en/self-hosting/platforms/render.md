---
title: Render
description: Deploy Kaeso on Render from the blueprint, with Render Postgres and Key Value.
order: 1
---

`render.yaml` at the root of the repository is a [Render Blueprint](https://render.com/docs/infrastructure-as-code). It creates a public edge, a private gateway and console, a Postgres instance and a Key Value instance, all pinned to one release.

## What the blueprint creates

| Service | Type | Image | Plan |
| --- | --- | --- | --- |
| `kaeso-edge` | Web service | The console image, running `kaeso-edge` | 0.5 CPU, 512 MB |
| `kaeso-gateway` | Private service | The gateway image | 2 CPU, 4 GB |
| `kaeso-console` | Private service | The console image | 0.5 CPU, 512 MB |
| `kaeso-valkey` | Key Value | Render's, with `noeviction` | 256 MB |
| `kaeso-postgres` | Postgres | Render's, version 18 | 1 CPU, 2 GB |

Only the edge is public. Render's private network carries requests from the edge to the gateway and the console, and from the gateway to Postgres and Key Value, whose `ipAllowList` is empty. Every service has `autoDeployTrigger: off`, so Render never redeploys on a commit. Change a plan or the region in the blueprint before you create it; Render keeps them from then on.

## Deploy

1. Generate a master key, and keep it in a password manager:

   ```bash
   docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate
   ```

2. In Render, create a **Blueprint** from the repository `logicplanes/kaeso`, branch `prod`, which holds the latest release. Render shows the values the blueprint asks for:

   | Value | What to enter |
   | --- | --- |
   | `KAESO_PUBLIC_URL` | `https://` and the edge's address, such as `https://kaeso-edge.onrender.com`, or your domain once it is set up |
   | `KAESO_MASTER_KEYS` | The key from step 1 |
   | `KAESO_SETUP_EMAIL` | Your address; the gateway emails the setup link there |
   | `KAESO_SMTP_HOST`, `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD`, `KAESO_SMTP_FROM` | Your SMTP server ([email](../email.md)) |
   | `KAESO_CLICKHOUSE_URL` | A ClickHouse HTTP URL with user and password for request logs, or empty to keep them off |

3. Wait until every service is live. The gateway emails the one-time setup link to `KAESO_SETUP_EMAIL`; open it to create the first user ([the first start](../how-it-works.md#the-first-start)).
4. For your own domain, add it to `kaeso-edge` under **Settings**, point its DNS record where Render says, set `KAESO_PUBLIC_URL` on `kaeso-gateway` to `https://` and the domain, and redeploy the gateway.

The gateway listens on port 8080 and keeps its metrics on its loopback address, since Render takes the one port a private service binds on every interface as its port. The edge trusts `X-Forwarded-For` from Render's private network, where its own proxy sits, and the gateway trusts the edge.

## Update and roll back

A release changes the image tags in `render.yaml`. On the blueprint's page, **Manual sync** redeploys the services on the new release; the gateway migrates the databases at start, forward only ([upgrades](../upgrades.md)). To roll back, use **Rollback** on each service to its previous deploy; a release whose migrations the earlier version can't run stays, as on any install.

## What this is based on

The blueprint's shape, the service types, the plan names, `fromService` and `fromDatabase` references and `ipAllowList` follow Render's blueprint specification, read on 2026-11-06. The three images were tested in this layout locally with Docker ([what was validated](index.md#what-was-validated)). The blueprint has not yet been deployed on Render from an empty account; do that in a disposable workspace first, and check the setup email, a streamed completion, the client address in a request log, and an update.

## Next steps

- [Configuration](../configuration.md): Every setting the gateway reads, and what each one does.
- [Backups and master keys](../backups.md): Back up Postgres on Render and keep the master key outside it.
- [Health and metrics](../monitoring.md): The health checks the edge and the gateway answer.
