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
| `compose.yaml` | The services, from the Kaeso release. Replace it with the new release's file to upgrade; the optional [updater](upgrades.md#one-click-updates) changes only its image versions. |
| `.env` | Your settings: the domain, the database passwords, email. Docker Compose reads it from the same folder. |
| `master-keys` | The master keys, handed to the gateway as the Docker secret `master_keys`. |
| `compose.override.yaml` | Optional: your own changes to the services, kept when `compose.yaml` is replaced. |

Create the folder, download `compose.yaml`, write `.env` with random passwords and generate a master key:

```bash
mkdir -m 700 kaeso && cd kaeso
curl -fsSLO https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml
cat > .env <<EOF
KAESO_DOMAIN=kaeso.example.com
POSTGRES_PASSWORD=$(openssl rand -hex 24)
POSTGRES_SUPERUSER_PASSWORD=$(openssl rand -hex 24)
CLICKHOUSE_PASSWORD=$(openssl rand -hex 24)
VALKEY_PASSWORD=$(openssl rand -hex 24)
EOF
docker run --rm ghcr.io/logicplanes/kaeso-gateway:latest master-key generate > master-keys
chmod 644 master-keys
```

The gateway runs as an unprivileged user inside its container, so `master-keys` must be readable; the folder's mode keeps it closed to other users of the server. Keep copies of `.env` and `master-keys` apart from the database backups; see [backups and master keys](backups.md).

The `curl` above fetches the newest stable release. To install a particular release, download from its tag instead, such as `https://github.com/logicplanes/kaeso/releases/download/v{{version}}/compose.yaml`. Each release also attaches [`env.example`](https://github.com/logicplanes/kaeso/releases/latest/download/env.example), every variable of `.env` with an example value, and [`kaeso.example.toml`](https://github.com/logicplanes/kaeso/releases/latest/download/kaeso.example.toml), every setting of the gateway.

## The .env file

`compose.yaml` reads these variables from `.env`:

| Variable | Required | What it sets |
| --- | --- | --- |
| `KAESO_DOMAIN` | yes | The domain Caddy serves and gets a certificate for. The public URL is `https://` and the domain. |
| `POSTGRES_PASSWORD` | yes | The password of PostgreSQL's `kaeso` user, which the gateway signs in as. It owns the `kaeso` database and is not a superuser. |
| `POSTGRES_SUPERUSER_PASSWORD` | yes | The password of PostgreSQL's superuser `postgres`, for your own administration. Kaeso never uses it, and `docker compose exec postgres psql -U postgres` signs in without it. |
| `CLICKHOUSE_PASSWORD` | yes | The password of ClickHouse's `kaeso` user. |
| `VALKEY_PASSWORD` | yes | The password Valkey requires on every connection. |
| `KAESO_PUBLIC_URL` | no | The public URL, when it is not `https://` and `KAESO_DOMAIN`, for example behind your own proxy. |
| `KAESO_SMTP_HOST`, `KAESO_SMTP_PORT`, `KAESO_SMTP_TLS`, `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD`, `KAESO_SMTP_FROM` | `KAESO_SMTP_HOST`, unless email is only logged | How the gateway sends email. See [email](email.md). |
| `KAESO_MAIL_TRANSPORT` | no | `log` to only log emails instead of sending them, for trying Kaeso. |
| `KAESO_UPDATER_TOKEN`, `COMPOSE_PROFILES=updater` | no | The secret the gateway and the updater share, and the profile that starts the updater, for [one-click updates](upgrades.md#one-click-updates). |
| `DOCKER_SOCKET` | no | Where Docker's socket is, for the updater's proxy, when it isn't `/var/run/docker.sock`. |

Use letters and digits only in the passwords, as `openssl rand -hex` writes them, because they go into connection URLs. PostgreSQL and ClickHouse take their passwords when their volumes are first created; changing them in `.env` later doesn't change the passwords the databases have. Valkey reads its password from `.env` at every start.

Compose passes only these variables on. Every other [setting](configuration.md) goes in `compose.override.yaml`, as [local changes](#local-changes) shows.

## What happens at the first start

Start the install, then print the setup link:

```bash
docker compose up -d --wait
docker compose exec gateway kaeso setup-link
```

With `--wait`, Compose returns once every service reports healthy, and fails when one doesn't. Caddy counts as healthy as soon as it serves, so the certificate for a new domain may still be on its way; see [certificates](domains-and-tls.md#certificates). Without `--wait`, Compose returns as soon as the containers exist; behind [your own reverse proxy](domains-and-tls.md#your-own-reverse-proxy), where no Caddy waits for the gateway, that is before `kaeso setup-link` can work.

1. PostgreSQL, ClickHouse and Valkey start. The gateway waits until PostgreSQL and Valkey report healthy.
2. The gateway checks its settings. A missing or invalid one stops it with a message that names the setting, never its value.
3. It migrates PostgreSQL before it serves anything, and ClickHouse in the background, retrying until ClickHouse answers; until then `/readyz` reports `degraded`. Migrations run under a lock, so only one gateway instance migrates while the others wait.
4. While the install has no users, the gateway logs that it isn't set up yet and how to get the setup link, but never the link itself: anyone who can read the log could take the install over with it. `kaeso setup-link` prints the link in your terminal. It works once, for 24 hours, and printing a new one retires the older ones.
5. Once the gateway and the console are healthy, Caddy starts, reports healthy as soon as it serves, and gets a certificate for `KAESO_DOMAIN` in the background.

The setup link creates the first user, who becomes an instance admin, the owner of the first organization and Admin of its default project. Everyone else joins by invitation, or signs up where the instance allows it; see [instance administration](../console/instance-administration.md). An install that should sign in through single sign-on alone is set up as [an install without passwords](sign-in.md#an-install-without-passwords) describes.

## Trying it on your computer

Set `KAESO_DOMAIN=localhost` and open `https://localhost`. Caddy serves it with a certificate from its own local authority, which your browser asks you to accept once. Without a mail server at hand, add `KAESO_MAIL_TRANSPORT=log` to `.env`.

curl and the SDKs don't trust Caddy's local authority. Copy its root certificate out of the container and pass it to them:

```bash
docker compose cp caddy:/data/caddy/pki/authorities/local/root.crt caddy-root.crt
curl --cacert caddy-root.crt https://localhost/v1/models -H "Authorization: Bearer $KAESO_API_KEY"
```

The Python SDKs read it from `SSL_CERT_FILE=caddy-root.crt`, the TypeScript SDKs from `NODE_EXTRA_CA_CERTS=caddy-root.crt`.

## Local changes

Put your own changes to the services in `compose.override.yaml` next to `compose.yaml`. Docker Compose merges the two files, and an upgrade that replaces `compose.yaml` keeps your changes:

```yaml
services:
  gateway:
    environment:
      KAESO_LOG_LEVEL: info,kaeso_server=debug
      KAESO_INSTANCE_ACCOUNT_CREATION: everyone
```

[Configuration](configuration.md) lists every setting. Apply changes with `docker compose up -d --wait`; Compose recreates only the services whose settings changed.

## The command line

The gateway image is also Kaeso's command line. Run its commands in the running container, where they use the gateway's settings:

```bash
docker compose exec gateway kaeso user reset-2fa owner@example.com
```

| Command | What it does |
| --- | --- |
| `kaeso setup-link` | Prints a new setup link while the install has no users; older links stop working |
| `kaeso master-key generate` | Prints a new master key as `id:base64` |
| `kaeso master-key rewrap` | Re-encrypts stored secrets with the first master key and reports which key ids are still in use; see [rotating the master key](backups.md#rotating-the-master-key) |
| `kaeso user reset-2fa <email>` | Removes a user's two-factor authentication and recovery codes, ends their sessions, emails them and records it in the instance audit log |
| `kaeso user sign-in-link <email>` | Prints a one-time sign-in link for an active user, valid for 15 minutes, for when no other way in works; see [getting back in](sign-in.md#getting-back-in) |
| `kaeso migrate` | Runs the PostgreSQL and ClickHouse migrations, then exits |
| `kaeso serve` | Runs the gateway; the image's default command |
