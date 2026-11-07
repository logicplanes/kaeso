---
title: DigitalOcean
description: Deploy Kaeso on DigitalOcean App Platform from the app spec, with a dev PostgreSQL and a managed Valkey.
order: 3
---

`.do/app.yaml` in the repository is an [App Platform app spec](https://docs.digitalocean.com/products/app-platform/reference/app-spec/). It runs the gateway and the console as two services behind the app's edge, with a dev PostgreSQL database and a managed Valkey database, all pinned to one release.

## What the spec creates

| Component | Kind | Image | Size |
| --- | --- | --- | --- |
| `kaeso-gateway` | Service on port 8080 | The gateway image | `apps-s-1vcpu-2gb` |
| `kaeso-console` | Service on port 3000 | The console image | `apps-s-1vcpu-0.5gb` |
| `kaeso-postgres` | Dev database, PostgreSQL 18 | DigitalOcean's | 512 MB |
| `kaeso-valkey` | Managed Valkey database | DigitalOcean's, created first | Your choice |

The app's edge terminates TLS and routes by path: `/v1`, `/platform`, `/mcp`, `/oauth` and `/.well-known` reach the gateway, every other path the console, so no `kaeso-edge` runs here. The gateway trusts `X-Forwarded-For` from the private network, where the edge sits, and keeps its metrics on its loopback address. Managed Valkey speaks TLS with a certificate of DigitalOcean's own authority, which the spec hands to the gateway as `KAESO_VALKEY_CA` ([configuration](../configuration.md#settings)).

The **Deploy to DO** button deploys one service with a dev database and nothing else, so Kaeso, which needs two services and a Valkey, is created with `doctl` instead.

## Deploy

1. Generate a master key, and keep it in a password manager:

   ```bash
   docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate
   ```

2. Install [`doctl`](https://docs.digitalocean.com/reference/doctl/), sign in, and create the Valkey database in the app's region:

   ```bash
   doctl auth init
   doctl databases create kaeso-valkey --engine valkey --region fra1 --size db-s-1vcpu-1gb --num-nodes 1 --wait
   ```

3. In the database's **Settings** in the control panel, set **Eviction policy** to **No eviction**, so a full database refuses writes instead of dropping limit counters. The same through the API, with the database's id:

   ```bash
   curl -X PUT -H "Authorization: Bearer $DIGITALOCEAN_TOKEN" -H "Content-Type: application/json" \
     -d '{"eviction_policy":"noeviction"}' "https://api.digitalocean.com/v2/databases/$DATABASE_ID/eviction_policy"
   ```

4. Copy `.do/app.yaml` from the release you deploy and fill in the values it leaves empty:

   | Value | What to enter |
   | --- | --- |
   | `KAESO_MASTER_KEYS` | The key from step 1 |
   | `KAESO_SETUP_EMAIL` | Your address; the gateway emails the setup link there |
   | `KAESO_SMTP_HOST`, `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD`, `KAESO_SMTP_FROM` | Your SMTP server ([email](../email.md)) |
   | `KAESO_CLICKHOUSE_URL` | A ClickHouse HTTP URL with user and password for request logs, or empty to keep them off |

   `KAESO_PUBLIC_URL` is `${APP_URL}`, the app's primary address, and the database addresses come from the databases, so none of them is typed. App Platform encrypts a `SECRET` value when it first receives it.

5. Create the app and wait for its first deployment:

   ```bash
   doctl apps create --spec app.yaml --wait
   ```

6. The gateway emails the one-time setup link to `KAESO_SETUP_EMAIL`; open it to create the first user ([the first start](../how-it-works.md#the-first-start)).
7. For your own domain, add it under `domains` in the spec with `type: PRIMARY`, apply the spec with `doctl apps update`, and point the DNS record where the app's **Networking** tab says. `${APP_URL}` follows the primary domain, so the gateway redeploys with the new address.

The dev database costs little and keeps no backups. For production, create a managed PostgreSQL cluster and point `kaeso-postgres` at it with `production: true` and its `cluster_name` ([backups and master keys](../backups.md)).

## Update and roll back

A release changes the image tags in `.do/app.yaml`. Apply the new spec, and the app deploys both services; the gateway migrates the databases at start, forward only ([upgrades](../upgrades.md)):

```bash
doctl apps update $APP_ID --spec app.yaml --wait
```

To roll back, choose **Rollback** beside an earlier deployment on the app's **Activity** tab; the ten most recent successful deployments are kept. A release whose migrations the earlier version can't run stays, as on any install.

## What this is based on

The spec's shape, the image, ingress, health check, env and database fields and the bindable `${APP_URL}`, `${…DATABASE_URL}` and `${…CA_CERT}` follow the app spec reference and the environment variable, container image and database pages, read on 2026-11-07. The two images were tested behind a path-routing edge locally with Docker ([what was validated](index.md#what-was-validated)). The spec has not yet been deployed on App Platform from an empty account; do that in a disposable project first, and check the setup email, a streamed completion that outlasts a minute, since the edge speaks HTTP/1.1 to the gateway, the client address in a request log, and an update.

## Next steps

- [Configuration](../configuration.md): Every setting the gateway reads, and what each one does.
- [Backups and master keys](../backups.md): Move to a managed PostgreSQL cluster with backups, and keep the master key outside the platform.
- [Health and metrics](../monitoring.md): The health checks the gateway answers.
