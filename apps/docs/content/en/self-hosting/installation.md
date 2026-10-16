---
title: Installation
description: The files of an install, its first start, local changes and the command line.
order: 2
---

The [quick start](../get-started/quick-start.md) is the short version of this page.

## The files

An install is a folder that only you can read, with these files:

| File | What it holds |
| --- | --- |
| `compose.yaml` | The services, from the Kaeso release. Replace it with the new release's file to upgrade. |
| `.env` | Your settings: the domain, the database passwords, email. Docker Compose reads it from the same folder. |
| `master-keys` | The master keys, handed to the gateway as the Docker secret `master_keys`. |
| `compose.override.yaml` | Optional: your own changes to the services, kept when `compose.yaml` is replaced. |

```bash
mkdir -m 700 kaeso && cd kaeso
curl -fsSLO https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml
printf 'KAESO_DOMAIN=kaeso.example.com\nPOSTGRES_PASSWORD=%s\nCLICKHOUSE_PASSWORD=%s\n' "$(openssl rand -hex 24)" "$(openssl rand -hex 24)" > .env
docker run --rm ghcr.io/logicplanes/kaeso-gateway:latest master-key generate > master-keys
chmod 644 master-keys
```

To install a particular release, download its file instead: `https://github.com/logicplanes/kaeso/releases/download/v0.1.0/compose.yaml`. Each release also carries `env.example`, every variable of `.env` with an example value, and `kaeso.example.toml`, every setting of the gateway. The file in the repository, [`deploy/compose.yaml`](https://github.com/logicplanes/kaeso/blob/dev/deploy/compose.yaml), is the one the next release will carry.

The gateway runs as an unprivileged user inside its container and reads `master-keys` from there, so the file itself is readable; the folder keeps it closed to other users of the server. Back up `.env` and `master-keys` apart from the database backups; see [backups and master keys](backups.md).

## The .env file

`compose.yaml` reads these variables from `.env`:

| Variable | Required | What it sets |
| --- | --- | --- |
| `KAESO_DOMAIN` | yes | The domain Caddy serves and gets a certificate for. The public URL is `https://` and the domain. |
| `POSTGRES_PASSWORD` | yes | The password of PostgreSQL's `kaeso` user. |
| `CLICKHOUSE_PASSWORD` | yes | The password of ClickHouse's `kaeso` user. |
| `KAESO_PUBLIC_URL` | no | The public URL, when it is not `https://` and `KAESO_DOMAIN`, for example behind your own proxy. |
| `KAESO_SMTP_HOST`, `KAESO_SMTP_PORT`, `KAESO_SMTP_TLS`, `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD`, `KAESO_SMTP_FROM` | `KAESO_SMTP_HOST`, unless email is only logged | How the gateway sends email. See [email](email.md). |
| `KAESO_MAIL_TRANSPORT` | no | `log` to only log emails instead of sending them, for trying Kaeso. |

The database passwords go into connection URLs, so use letters and digits only, as `openssl rand -hex` writes them. They take effect when the databases are first created in their volumes; changing them later in `.env` doesn't change the passwords the databases already have.

Compose reads only these variables from `.env`. Every other [setting](configuration.md) goes in `compose.override.yaml`, as [local changes](#local-changes) shows.

## What happens at the first start

```bash
docker compose up -d
docker compose logs gateway | grep setup_url
```

`docker compose up -d` returns once the gateway is healthy. The setup link is the `setup_url` of the line it finds.

1. PostgreSQL, ClickHouse and Valkey start; Compose waits until PostgreSQL and Valkey report healthy.
2. The gateway checks its settings and stops with a message that names the setting if one is missing or wrong. It never prints secret values.
3. It migrates PostgreSQL to its version before it serves anything, and ClickHouse in the background, retrying until ClickHouse answers; until then `/readyz` reports `degraded`. Migrations run under a lock, so only one gateway instance migrates while others wait.
4. While the install has no users, it logs a setup link. The link works once, for 24 hours. Each restart, and `docker compose exec gateway kaeso setup-link`, prints a new one and retires the old ones.
5. Once the gateway and the console are healthy, Caddy starts and gets a certificate for `KAESO_DOMAIN`.

The setup page creates the first user, who becomes an instance admin and the owner of the first organization, with a default project where they are Admin. After that, people join by invitation, or by signing up where the instance allows it; see [instance administration](../console/instance-administration.md).

## Trying it on your computer

Set `KAESO_DOMAIN=localhost` and open `https://localhost`. Caddy serves it with a certificate from its own local authority, which your browser asks you to accept once. Add `KAESO_MAIL_TRANSPORT=log` to `.env` if you have no mail server at hand.

## Local changes

To change a setting of a service, or add one, put it in `compose.override.yaml` next to `compose.yaml`. Docker Compose merges the two, and an upgrade that replaces `compose.yaml` keeps your changes.

```yaml
services:
  gateway:
    environment:
      KAESO_LOG_LEVEL: info,kaeso_server=debug
      KAESO_INSTANCE_ACCOUNT_CREATION: everyone
```

Every setting is listed under [configuration](configuration.md). Apply changes with `docker compose up -d`; Compose recreates only the services whose settings changed.

## The command line

The gateway image is also its command line. Run its commands in the running container, where they use the gateway's settings:

| Command | What it does |
| --- | --- |
| `kaeso setup-link` | Prints a new setup link while the install has no users; older links stop working |
| `kaeso master-key generate` | Prints a new master key as `id:base64` |
| `kaeso master-key rewrap` | Re-encrypts stored secrets with the first master key and reports which key ids are still in use; see [rotating the master key](backups.md#rotating-the-master-key) |
| `kaeso user reset-2fa <email>` | Removes a user's two-factor authentication and recovery codes, ends their sessions and emails them |
| `kaeso migrate` | Runs the PostgreSQL and ClickHouse migrations, then exits |
| `kaeso serve` | Runs the gateway; the image's default command |

For example:

```bash
docker compose exec gateway kaeso user reset-2fa owner@example.com
```
