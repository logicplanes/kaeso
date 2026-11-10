---
title: Coolify
description: Deploy Kaeso on Coolify from the Compose file, with Coolify's proxy and generated passwords.
order: 8
---

`deploy/coolify/compose.yaml` in the repository is a Docker Compose file for [Coolify](https://coolify.io/docs/knowledge-base/docker/compose). It runs a public edge behind Coolify's proxy, a private gateway and console, and the Compose package's PostgreSQL, ClickHouse and Valkey on your Coolify server, all pinned to one release. Coolify generates every password and the master key.

## What the file runs

| Service | Image | Reached by |
| --- | --- | --- |
| `edge` | The console image, running `kaeso-edge` | Everyone, through Coolify's proxy on the domain you give it |
| `gateway` | The gateway image | The edge |
| `console` | The console image | The edge |
| `postgres`, `clickhouse`, `valkey` | The releases the Compose package runs | The gateway |

Each service keeps the Compose package's hardening: no new privileges, all Linux capabilities dropped but the ones a data store needs, a read-only root file system for Kaeso's services, and memory and process limits. The gateway signs in to PostgreSQL as `kaeso`, which owns its database but isn't a superuser. Valkey never evicts a key, so a full Valkey refuses writes instead of dropping limit counters.

Coolify fills in the values whose names start with `SERVICE_`: `SERVICE_URL_EDGE_8080` is the edge's address on port 8080, the `SERVICE_PASSWORD_…` values are random passwords, and `SERVICE_REALBASE64_32_MASTERKEY` is 32 random bytes, the master key.

## Deploy

1. In Coolify, open a project and add a resource of the type **Docker Compose Empty**. Paste the file from the branch `prod`, which holds the latest release: `https://raw.githubusercontent.com/logicplanes/kaeso/prod/deploy/coolify/compose.yaml`.
2. Under the `edge` service, set the domain to `https://` and your domain, such as `https://kaeso.example.com`, and point its DNS record at the server. Kaeso needs an `https` address.
3. Under **Environment Variables**, fill in your SMTP server, if you have one ([email](../email.md)):

   | Variable | What to enter |
   | --- | --- |
   | `KAESO_MAIL_TRANSPORT` | `smtp` to send email; `log`, the default, only logs the kind of each email |
   | `KAESO_SETUP_EMAIL` | Your address; with SMTP, the gateway emails the setup link there |
   | `KAESO_SMTP_HOST`, `KAESO_SMTP_PORT`, `KAESO_SMTP_TLS`, `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD`, `KAESO_SMTP_FROM` | Your SMTP server |

4. Deploy, and wait until every service is healthy.
5. Copy `SERVICE_REALBASE64_32_MASTERKEY` from **Environment Variables** into a password manager; the master key is `k1:` and that value. Kaeso encrypts provider keys with it, so a database backup can't be read without it ([backups and master keys](../backups.md)).
6. Open the setup link to create the first user ([the first start](../how-it-works.md#the-first-start)). With SMTP, it arrives at `KAESO_SETUP_EMAIL`. Otherwise, print it on the server, in a terminal or Coolify's terminal for the server:

   ```bash
   docker exec $(docker ps --quiet --filter name=gateway-) kaeso setup-link
   ```

The gateway image has no shell, so Coolify's terminal for the gateway's container can't run the command. On a server with several Kaeso resources, give `docker exec` the gateway container that `docker ps` lists for this one.

## Update and roll back

A release changes the image tags in `compose.yaml`. Open the resource's Compose file in Coolify, set the tags of `edge`, `gateway` and `console` to the new release, and deploy; Coolify keeps the generated values. The gateway migrates the databases at start, forward only ([upgrades](../upgrades.md)). To roll back, set the earlier tags; a release whose migrations the earlier version can't run stays, as on any install.

## Limits

- Kaeso's updater doesn't run here, so the console can't update Kaeso; you deploy each release in Coolify.
- Coolify's proxy terminates TLS. The gateway writes a keepalive into every stream, so long streams outlast the proxy's idle timeout ([long streams](../domains-and-tls.md#long-streams)).

## What this is based on

The file follows Coolify's pages on Docker Compose and on contributing a service, and its magic variables as Coolify's source generates them, read on 2026-11-09. `docker compose config` accepts it. On 2026-11-09 it ran locally with Docker, with the values Coolify generates filled in by hand: every service turned healthy, the edge sent the APIs to the gateway and every other path to the console, `kaeso setup-link` printed the link, and the `kaeso` user wasn't a PostgreSQL superuser. It hasn't been deployed on Coolify. Do that on a disposable server first, and check the setup link, a streamed completion and the client address in a request log ([what was validated](index.md#what-was-validated)).

## Next steps

- [Configuration](../configuration.md): Every setting the gateway reads, and what each one does.
- [Backups and master keys](../backups.md): Back up PostgreSQL and keep the master key outside the server.
- [Health and metrics](../monitoring.md): The health checks the services answer.
