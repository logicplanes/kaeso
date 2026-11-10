---
title: Dokploy
description: Deploy Kaeso on Dokploy by importing its template, with Traefik and generated passwords.
order: 9
---

`deploy/dokploy/` in the repository holds a [Dokploy template](https://github.com/Dokploy/templates): `docker-compose.yml` and `template.toml`. It runs a public edge behind Dokploy's Traefik, a private gateway and console, and the Compose package's PostgreSQL, ClickHouse and Valkey on your Dokploy server, all pinned to one release. The template generates every password and the master key.

## What the template runs

| Service | Image | Reached by |
| --- | --- | --- |
| `edge` | The console image, running `kaeso-edge` | Everyone, through Traefik on the template's domain |
| `gateway` | The gateway image | The edge |
| `console` | The console image | The edge |
| `postgres`, `clickhouse`, `valkey` | The releases the Compose package runs | The gateway |

Each service keeps the Compose package's hardening: no new privileges, all Linux capabilities dropped but the ones a data store needs, a read-only root file system for Kaeso's services, and memory and process limits. The gateway signs in to PostgreSQL as `kaeso`, which owns its database but isn't a superuser. Valkey never evicts a key, so a full Valkey refuses writes instead of dropping limit counters.

`template.toml` gives the edge a domain on port 8080 and sets the environment: random passwords from Dokploy's `hash` helper, and a master key from its `base64` helper, 32 random bytes. Mail is only logged until you add an SMTP server.

## Deploy

1. On a computer with `curl` and `jq`, turn the template of the branch `prod`, which holds the latest release, into the Base64 text Dokploy imports:

   ```bash
   curl -fsSLO https://raw.githubusercontent.com/logicplanes/kaeso/prod/deploy/dokploy/docker-compose.yml
   curl -fsSLO https://raw.githubusercontent.com/logicplanes/kaeso/prod/deploy/dokploy/template.toml
   jq -n --rawfile compose docker-compose.yml --rawfile config template.toml '{compose: $compose, config: $config}' | base64 | tr -d '\n'
   ```

2. In Dokploy, open a project and create a **Compose** service. Under **Advanced**, paste the text into **Import** and import it. Dokploy shows the Compose file, the environment, the mounts and the domain before it applies them.
3. Under **Domains**, change the host to your domain, such as `kaeso.example.com`, turn on HTTPS with Let's Encrypt, and point the domain's DNS record at the server.
4. Under **Environment**, set `KAESO_PUBLIC_URL` to `https://` and that domain, and fill in your SMTP server, if you have one ([email](../email.md)):

   | Variable | What to enter |
   | --- | --- |
   | `KAESO_MAIL_TRANSPORT` | `smtp` to send email; `log`, the default, only logs the kind of each email |
   | `KAESO_SETUP_EMAIL` | Your address; with SMTP, the gateway emails the setup link there |
   | `KAESO_SMTP_HOST`, `KAESO_SMTP_PORT`, `KAESO_SMTP_TLS`, `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD`, `KAESO_SMTP_FROM` | Your SMTP server |

5. Deploy, and wait until every service is healthy.
6. Copy `KAESO_MASTER_KEYS` from **Environment** into a password manager. Kaeso encrypts provider keys with it, so a database backup can't be read without it ([backups and master keys](../backups.md)).
7. Open the setup link to create the first user ([the first start](../how-it-works.md#the-first-start)). With SMTP, it arrives at `KAESO_SETUP_EMAIL`. Otherwise, print it on the server, with the service's app name from its **General** tab in place of `kaeso-example`:

   ```bash
   docker exec kaeso-example-gateway-1 kaeso setup-link
   ```

The gateway image has no shell, so Dokploy's terminal for the gateway's container can't run the command.

## Update and roll back

A release changes the image tags in `docker-compose.yml`. Open the service's Compose file in Dokploy, set the tags of `edge`, `gateway` and `console` to the new release, and deploy; the environment stays. The gateway migrates the databases at start, forward only ([upgrades](../upgrades.md)). To roll back, set the earlier tags; a release whose migrations the earlier version can't run stays, as on any install.

## Limits

- Kaeso's updater doesn't run here, so the console can't update Kaeso; you deploy each release in Dokploy.
- Traefik terminates TLS. The gateway writes a keepalive into every stream, so long streams outlast the proxy's idle timeout ([long streams](../domains-and-tls.md#long-streams)).

## What this is based on

The template follows the format of Dokploy's template repository and Dokploy's source for importing a template and for its helpers, read on 2026-11-09. `docker compose config` accepts the Compose file with the template's environment. On 2026-11-09 it ran locally with Docker, with that environment filled in by hand: every service turned healthy, the edge sent the APIs to the gateway and every other path to the console, the gateway migrated ClickHouse, `kaeso setup-link` printed the link, and the `kaeso` user wasn't a PostgreSQL superuser. It hasn't been imported into Dokploy. Do that on a disposable server first, and check the setup link, a streamed completion and the client address in a request log ([what was validated](index.md#what-was-validated)).

## Next steps

- [Configuration](../configuration.md): Every setting the gateway reads, and what each one does.
- [Backups and master keys](../backups.md): Back up PostgreSQL and keep the master key outside the server.
- [Health and metrics](../monitoring.md): The health checks the services answer.
