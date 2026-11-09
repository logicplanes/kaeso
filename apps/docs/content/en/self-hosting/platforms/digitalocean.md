---
title: DigitalOcean
description: Deploy Kaeso on DigitalOcean App Platform with the Deploy to DigitalOcean button or from the app spec.
order: 3
---

Two [App Platform app specs](https://docs.digitalocean.com/products/app-platform/reference/app-spec/) in the repository run the gateway and the console as two services behind the app's edge, pinned to one release. `.do/deploy.template.yaml` is what the **Deploy to DigitalOcean** button reads: it runs Valkey inside the app. `.do/app.yaml` uses a managed Valkey database instead, and you create it with `doctl`.

## What the specs create

| Component | Kind | Image | Size |
| --- | --- | --- | --- |
| `kaeso-gateway` | Service on port 8080 | The gateway image | `apps-s-1vcpu-2gb` |
| `kaeso-console` | Service on port 3000 | The console image | `apps-s-1vcpu-0.5gb` |
| `kaeso-postgres` | Dev database, PostgreSQL 18 | DigitalOcean's | 512 MB |
| `kaeso-valkey`, button | Service on port 6379, for the app's own components only | Valkey's, the release the Compose package runs | `apps-s-1vcpu-0.5gb` |
| `kaeso-valkey`, `doctl` | Managed Valkey database | DigitalOcean's, created first | Your choice |

The app's edge terminates TLS and routes by path: `/v1`, `/v1beta`, `/v2`, `/platform`, `/mcp`, `/oauth` and `/.well-known` reach the gateway, every other path the console, so no `kaeso-edge` runs here. The gateway trusts `X-Forwarded-For` from the private network, where the edge sits, and keeps its metrics on its loopback address.

The button creates dev databases only, and those exist only for PostgreSQL. Its Valkey is therefore a service in the app that keeps its data within 256 MB and never evicts a key. It starts empty at each of its deployments. Valkey holds only counters and caches: the gateway rebuilds budget counters from PostgreSQL, and rate windows start over ([backups and master keys](../backups.md)). The managed Valkey of `.do/app.yaml` speaks TLS with a certificate of DigitalOcean's own authority, which the spec hands to the gateway as `KAESO_VALKEY_CA` ([configuration](../configuration.md#settings)).

## Values to fill

Both specs leave these values empty:

| Value | What to enter |
| --- | --- |
| `KAESO_MASTER_KEYS` | A master key from `docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate`, also kept in a password manager |
| `KAESO_SETUP_EMAIL` | Your address; the gateway emails the setup link there |
| `KAESO_SMTP_HOST`, `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD`, `KAESO_SMTP_FROM` | Your SMTP server ([email](../email.md)) |
| `KAESO_CLICKHOUSE_URL` | A ClickHouse HTTP URL with user and password for request logs, or empty to keep them off |

`KAESO_PUBLIC_URL` is `${APP_URL}`, the app's primary address, and the database addresses come from the databases, so none of them is typed. App Platform encrypts a `SECRET` value when it first receives it.

## Deploy with the button

1. Generate a master key, and keep it in a password manager:

   ```bash
   docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate
   ```

2. Open [Deploy to DigitalOcean](https://cloud.digitalocean.com/apps/new?repo=https://github.com/logicplanes/kaeso/tree/prod&refcode=881255e24b11). App Platform reads the spec from the branch `prod`, which holds the latest release.
3. In the environment variables of `kaeso-gateway`, fill in the [values](#values-to-fill), and choose another region if you like.
4. Create the app and wait until its first deployment is live.
5. The gateway emails the one-time setup link to `KAESO_SETUP_EMAIL`; open it to create the first user ([the first start](../how-it-works.md#the-first-start)).
6. For your own domain, add it in the app's **Settings** under **Domains**, and point the DNS record where App Platform says. `${APP_URL}` follows the primary domain, so the gateway redeploys with the new address.

> [!IMPORTANT]
> The dev database keeps no backups. For production, deploy with `doctl`, or move the app to a managed PostgreSQL cluster and a managed Valkey as below.

## Deploy with doctl

1. Generate a master key as in the button's first step.
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

4. Copy `.do/app.yaml` from the release you deploy and fill in the [values](#values-to-fill).
5. Create the app and wait for its first deployment:

   ```bash
   doctl apps create --spec app.yaml --wait
   ```

6. Open the setup link from the email to create the first user.
7. For your own domain, add it under `domains` in the spec with `type: PRIMARY`, apply the spec with `doctl apps update`, and point the DNS record where the app's **Networking** tab says.

The dev database costs little and keeps no backups. For production, create a managed PostgreSQL cluster and point `kaeso-postgres` at it with `production: true` and its `cluster_name` ([backups and master keys](../backups.md)).

## Update and roll back

A release changes the image tags of `kaeso-gateway` and `kaeso-console` in both specs. Set the two tags in your app's spec and apply it, and the app deploys both services; the gateway migrates the databases at start, forward only ([upgrades](../upgrades.md)). `doctl apps spec get $APP_ID` prints the spec of an app the button created:

```bash
doctl apps spec get $APP_ID > app.yaml
doctl apps update $APP_ID --spec app.yaml --wait
```

To roll back, choose **Rollback** beside an earlier deployment on the app's **Activity** tab; the ten most recent successful deployments are kept. A release whose migrations the earlier version can't run stays, as on any install.

## What this is based on

The specs' shape, the image, ingress, health check, env, internal port and database fields and the bindable `${APP_URL}`, `${…DATABASE_URL}` and `${…CA_CERT}` follow the app spec reference and the environment variable, container image and database pages, read on 2026-11-07. The button's template follows DigitalOcean's page on the Deploy to DigitalOcean button, read on 2026-11-09. On that day, App Platform's validation and proposal API accepted the button's spec. The two images were tested behind a path-routing edge locally with Docker ([what was validated](index.md#what-was-validated)). Neither spec has yet been deployed on App Platform from an empty account. Do that in a disposable project first, and check the setup email and a streamed completion that outlasts a minute, since the edge speaks HTTP/1.1 to the gateway. Also check the client address in a request log, and an update.

## Next steps

- [Configuration](../configuration.md): Every setting the gateway reads, and what each one does.
- [Backups and master keys](../backups.md): Move to a managed PostgreSQL cluster with backups, and keep the master key outside the platform.
- [Health and metrics](../monitoring.md): The health checks the gateway answers.
