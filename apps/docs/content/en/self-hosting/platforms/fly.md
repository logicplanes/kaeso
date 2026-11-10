---
title: Fly.io
description: Deploy Kaeso on Fly.io as four apps on the private network, with Fly Managed Postgres.
order: 4
---

`deploy/fly/` in the repository holds one Fly Launch configuration per app. Fly.io runs one app per `fly.toml`, and Kaeso needs four: a public edge, a private gateway and console, and Valkey. Fly Managed Postgres holds the database. All four apps are pinned to one release.

## What the files create

| App | File | Image | Machine |
| --- | --- | --- | --- |
| `…-edge` | `edge.toml` | The console image, running `kaeso-edge` | `shared-cpu-1x`, 256 MB |
| `…-gateway` | `gateway.toml` | The gateway image | `shared-cpu-2x`, 2 GB |
| `…-console` | `console.toml` | The console image | `shared-cpu-1x`, 256 MB |
| `…-valkey` | `valkey.toml` | Valkey's, the release the Compose package runs, on a 1 GB volume | `shared-cpu-1x`, 512 MB |

Only the edge has a public service. The other apps have none, so they are reachable only at `<app>.internal` on the organization's private network. That network is IPv6, so the gateway listens on `[::]:8080`. The edge reads `Fly-Client-IP`, which Fly's proxy sets, rather than the caller-controlled `X-Forwarded-For`. The gateway trusts the private network and keeps its metrics on its loopback address.

Valkey keeps its data within 256 MB and never evicts a key, so a full Valkey refuses writes instead of dropping limit counters. It runs on one machine with one volume, which Fly doesn't replicate. Valkey holds only counters and caches: the gateway rebuilds budget counters from PostgreSQL, and rate windows start over ([backups and master keys](../backups.md)).

## Deploy

Fly.io has no deploy button for a project of several apps, so you deploy with `fly`, Fly's command-line tool.

1. Generate a master key into `KAESO_MASTER_KEYS`, and copy what `echo` prints into a password manager:

   ```bash
   export KAESO_MASTER_KEYS=$(docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate)
   echo $KAESO_MASTER_KEYS
   ```

2. Install [`fly`](https://fly.io/docs/flyctl/install/), sign in, and clone the branch `prod`, which holds the latest release:

   ```bash
   fly auth login
   git clone --branch prod https://github.com/logicplanes/kaeso.git
   cd kaeso
   ```

3. App names are unique across Fly.io, so choose a prefix of your own, and create the four apps:

   ```bash
   export KAESO_APP=kaeso-example
   fly apps create $KAESO_APP-edge
   fly apps create $KAESO_APP-gateway
   fly apps create $KAESO_APP-console
   fly apps create $KAESO_APP-valkey
   ```

4. Create a Managed Postgres cluster in the apps' region, and attach it to the gateway. `fly mpg list` shows the cluster's id:

   ```bash
   fly mpg create --name $KAESO_APP --region fra --pg-major-version 17
   fly mpg attach $CLUSTER_ID --app $KAESO_APP-gateway --variable-name KAESO_POSTGRES_URL
   ```

5. Set the secrets, with your SMTP server in place of the examples ([email](../email.md)):

   ```bash
   export VALKEY_PASSWORD=$(openssl rand -hex 24)
   fly secrets set --stage --app $KAESO_APP-valkey VALKEY_PASSWORD=$VALKEY_PASSWORD
   fly secrets set --stage --app $KAESO_APP-edge KAESO_GATEWAY_UPSTREAM=$KAESO_APP-gateway.internal:8080 KAESO_CONSOLE_UPSTREAM=$KAESO_APP-console.internal:3000
   fly secrets set --stage --app $KAESO_APP-gateway KAESO_PUBLIC_URL=https://$KAESO_APP-edge.fly.dev \
     KAESO_VALKEY_URL=redis://:$VALKEY_PASSWORD@$KAESO_APP-valkey.internal:6379 KAESO_MASTER_KEYS="$KAESO_MASTER_KEYS" \
     KAESO_SETUP_EMAIL=owner@example.com KAESO_SMTP_HOST=smtp.example.com KAESO_SMTP_USER=kaeso \
     KAESO_SMTP_PASSWORD=secret KAESO_SMTP_FROM="Kaeso <kaeso@example.com>"
   ```

6. Deploy Valkey and the console as one machine each, then the gateway and the edge:

   ```bash
   fly deploy --config deploy/fly/valkey.toml --app $KAESO_APP-valkey --ha=false
   fly deploy --config deploy/fly/console.toml --app $KAESO_APP-console --ha=false
   fly deploy --config deploy/fly/gateway.toml --app $KAESO_APP-gateway
   fly deploy --config deploy/fly/edge.toml --app $KAESO_APP-edge
   ```

7. The gateway emails the one-time setup link to `KAESO_SETUP_EMAIL`; open it to create the first user ([the first start](../how-it-works.md#the-first-start)). The gateway image has no shell for `kaeso setup-link`. If the email doesn't arrive, the gateway's logs name the SMTP error; correct the secrets, and the gateway sends a new link at its next start.
8. For your own domain, add a certificate to the edge, and point the DNS records where `fly certs add` says. Then set `KAESO_PUBLIC_URL` on the gateway to `https://` and the domain:

   ```bash
   fly certs add kaeso.example.com --app $KAESO_APP-edge
   fly secrets set --app $KAESO_APP-gateway KAESO_PUBLIC_URL=https://kaeso.example.com
   ```

For request logs, set `KAESO_CLICKHOUSE_URL` on the gateway to a ClickHouse HTTP URL with user and password, such as ClickHouse Cloud. Without it, request logs are off and usage, limits and budgets work the same ([choose how to keep request logs](../installation.md#choose-how-to-keep-request-logs)).

## Update and roll back

A release changes the image tags in `edge.toml`, `gateway.toml` and `console.toml`. Pull the branch `prod` and deploy the gateway, then the console and the edge, with the commands of step 6. The gateway migrates the database at start, forward only ([upgrades](../upgrades.md)). A machine gets 75 seconds to drain when it stops. To roll back, deploy an earlier image, such as `fly deploy --config deploy/fly/gateway.toml --app $KAESO_APP-gateway --image ghcr.io/logicplanes/kaeso-gateway:X.Y.Z`; a release whose migrations the earlier version can't run stays, as on any install.

## Limits

- Fly Machines have no read-only root file system and no setting to drop Linux capabilities, so the apps run with the images' users only.
- Managed Postgres offers PostgreSQL 16 and 17; the Compose package runs 18. Kaeso runs on 17, as on Google Cloud.
- The gateway trusts every address of the organization's private network. Run Kaeso in an organization of its own when other apps there shouldn't be able to set the client address the gateway logs.

## What this is based on

The files follow Fly.io's app configuration reference, its request header and private networking pages, and the `fly mpg create` and `fly mpg attach` references, read on 2026-11-09. The four files parse as TOML and pass the [SchemaStore](https://www.schemastore.org/fly.json) schema for `fly.toml`, which lacks the top-level `[checks]` section that Fly's reference documents. `fly config validate` needs a Fly.io account, so it wasn't run. The apps haven't been deployed on Fly.io. Do that in a disposable organization first, and check the setup email, a streamed completion and the client address in a request log ([what was validated](index.md#what-was-validated)).

## Next steps

- [Configuration](../configuration.md): Every setting the gateway reads, and what each one does.
- [Backups and master keys](../backups.md): Back up Managed Postgres and keep the master key outside Fly.io.
- [Health and metrics](../monitoring.md): The health checks the edge and the gateway answer.
