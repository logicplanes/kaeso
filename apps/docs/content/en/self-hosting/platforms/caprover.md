---
title: CapRover
description: Deploy Kaeso on CapRover as a one-click app from the template in the repository.
order: 10
---

`deploy/caprover/kaeso.yml` in the repository is a [CapRover one-click app](https://github.com/caprover/one-click-apps). It creates six apps on your CapRover server: a public edge behind CapRover's nginx, a private gateway and console, and the Compose package's PostgreSQL, ClickHouse and Valkey, all pinned to one release.

## What the template creates

| App | Image | Reached by |
| --- | --- | --- |
| `kaeso` | The console image, running `kaeso-edge` | Everyone, through nginx on the app's domain |
| `kaeso-gateway` | The gateway image | The edge |
| `kaeso-console` | The console image | The edge |
| `kaeso-postgres`, `kaeso-clickhouse`, `kaeso-valkey` | The releases the Compose package runs | The gateway |

The names follow the app name you give, `kaeso` here. Only the edge is a web app; the others are reachable only on CapRover's overlay network. The gateway signs in to PostgreSQL as `kaeso`, which owns its database but isn't a superuser. Valkey keeps its data within 512 MB and never evicts a key, so a full Valkey refuses writes instead of dropping limit counters.

CapRover's nginx appends to the caller's `X-Forwarded-For`, so the edge reads `X-Real-IP`, which nginx sets. The template turns on websocket support for the edge, which makes nginx speak HTTP/1.1 to it. The gateway trusts only private addresses and keeps its metrics on its loopback address.

## Deploy

1. Generate a master key, and keep it in a password manager:

   ```bash
   docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate
   ```

2. In CapRover, open **Apps**, then **One-Click Apps/Databases**, and choose **>> TEMPLATE <<** at the end of the list.
3. Paste the template from the branch `prod`, which holds the latest release: `https://raw.githubusercontent.com/logicplanes/kaeso/prod/deploy/caprover/kaeso.yml`.
4. Fill in the form:

   | Field | What to enter |
   | --- | --- |
   | App name | `kaeso`, or a name of your own |
   | Public URL | The app's address under your root domain, which the form fills in, or your own domain, such as `https://kaeso.example.com` |
   | Master key | The key from step 1 |
   | Passwords | Random ones, which the form fills in |
   | Mail transport | `smtp` to send email; `log`, the default, only logs the kind of each email |
   | Setup email, SMTP host, port, user, password, sender | Your SMTP server, if you have one ([email](../email.md)) |

5. Deploy, and wait until the apps are created. The gateway restarts until PostgreSQL and Valkey answer.
6. Open the app `kaeso`, turn on **HTTPS** and **Force HTTPS**, and connect your domain if the public URL names one.
7. Open the setup link to create the first user ([the first start](../how-it-works.md#the-first-start)). With SMTP, it arrives at the setup email. Otherwise, print it on the server:

   ```bash
   docker exec $(docker ps --quiet --filter name=srv-captain--kaeso-gateway) kaeso setup-link
   ```

## Update and roll back

A release changes the image tags in `kaeso.yml`. In each of the apps `kaeso-gateway`, `kaeso-console` and `kaeso`, open **Deployment**, deploy by image name with the new tag, such as `ghcr.io/logicplanes/kaeso-gateway:{{version}}`, gateway first. The gateway migrates the databases at start, forward only ([upgrades](../upgrades.md)). To roll back, deploy the earlier tag the same way; a release whose migrations the earlier version can't run stays, as on any install.

## Limits

- CapRover reads only some Compose fields from a one-click app. Health checks, dropped capabilities, a read-only root file system and memory limits don't reach the apps, so they run with the images' defaults.
- nginx waits 60 seconds for a reply by default. Streams send keepalives, but a request without streaming that takes longer gets `504`. Raise `proxy_read_timeout` in the edge app's nginx configuration if you need more.
- Kaeso's updater doesn't run here, so the console can't update Kaeso; you deploy each release in CapRover.

## What this is based on

The template follows CapRover's guide to one-click apps, the Compose fields its source reads, and its nginx configuration, read on 2026-11-09. It parses as YAML, and every field it uses is one CapRover reads. It hasn't been deployed on CapRover. Do that on a disposable server first, and check the setup link, a streamed completion and the client address in a request log ([what was validated](index.md#what-was-validated)).

## Next steps

- [Configuration](../configuration.md): Every setting the gateway reads, and what each one does.
- [Backups and master keys](../backups.md): Back up PostgreSQL and keep the master key outside the server.
- [Domains and TLS](../domains-and-tls.md): Long streams, and what a proxy in front of Kaeso needs.
