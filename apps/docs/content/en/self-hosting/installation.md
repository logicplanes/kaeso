---
title: Installation
description: Install Kaeso by hand or a particular release, try it on your computer, change its services and run its commands.
order: 3
---

The [quick start](../get-started/quick-start.md) installs the newest release on a server with one command. This page shows each step that command takes, what you might do differently, and the commands you run on an install.

## Install by hand

The [installer](../get-started/quick-start.md#install-kaeso) takes these steps for you. Take them yourself to choose the folder, the passwords and the email settings as you go.

### Download the Compose file

Each release publishes a `compose.yaml` that pins its images by version and digest. Download it into a new folder that only you can read:

```bash
mkdir -m 700 kaeso && cd kaeso
curl -fsSLO \
  https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml
```

### Write the settings

Create `.env` with your domain and four random passwords. `umask 077` makes it, and every file you create in this shell from now on, readable only by you:

```bash
umask 077
cat > .env <<EOF
KAESO_DOMAIN=kaeso.example.com
POSTGRES_PASSWORD=$(openssl rand -hex 24)
POSTGRES_SUPERUSER_PASSWORD=$(openssl rand -hex 24)
CLICKHOUSE_PASSWORD=$(openssl rand -hex 24)
VALKEY_PASSWORD=$(openssl rand -hex 24)
EOF
```

The gateway sends sign-in links, invitations and alerts by email, and it doesn't start without a way to send them. Add your SMTP server. Kaeso reaches it on port 587 with STARTTLS unless [you change it](../self-hosting/email.md):

```bash
cat >> .env <<'EOF'
KAESO_SMTP_HOST=smtp.example.com
KAESO_SMTP_USER=kaeso
KAESO_SMTP_PASSWORD=…
KAESO_SMTP_FROM="Kaeso <kaeso@example.com>"
EOF
```

To try Kaeso without a mail server, let the gateway only log its emails. Password sign-in and the setup link still work, but sign-in links and invitations aren't delivered:

```bash
echo KAESO_MAIL_TRANSPORT=log >> .env
```

### Create the master key

The master key encrypts the provider keys that Kaeso stores. Generate one into `master-keys`, which Compose hands to the gateway as a Docker secret:

```bash
docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} \
  master-key generate > master-keys
sudo chown 65532:65532 master-keys && sudo chmod 0400 master-keys
```

The gateway runs as user 65532 in its container, so the file belongs to that user, and no one else can read it. Docker Desktop and Colima read the folder as you, so there run only `chmod 0400 master-keys`. With rootless Docker, give the file to the user that 65532 maps to on the host.

> [!IMPORTANT]
> Keep a copy of `master-keys` somewhere safe, apart from your database backups. Without it, Kaeso can't decrypt the stored provider keys, and you have to enter them again.

### Start Kaeso

Start every service and wait until each one is healthy:

```bash
docker compose up -d --wait
```

The first start takes a minute or two while the images download and the gateway prepares the databases. The command returns once every service is healthy, or names the one that isn't. Caddy requests the certificate in the background, so HTTPS requests can fail until it has one. `docker compose logs caddy` shows when it gets the certificate, or why it can't.

## Install a particular release

The installer downloads the `compose.yaml` of the newest stable release. To install another release, give the installer its version, such as `--version {{version}}`, or download the file from that release's tag:

```bash
curl -fsSLO \
  https://github.com/logicplanes/kaeso/releases/download/v{{version}}/compose.yaml
```

Each release also attaches two example files: [`env.example`](https://github.com/logicplanes/kaeso/releases/latest/download/env.example) with the usual variables of `.env`, and [`kaeso.example.toml`](https://github.com/logicplanes/kaeso/releases/latest/download/kaeso.example.toml) with every setting of the gateway.

## Trying it on your computer

Set `KAESO_DOMAIN=localhost` in `.env`, start Kaeso and open `https://localhost`. Caddy signs the certificate with its own local authority, which your browser asks you to accept once. Without a mail server, also set `KAESO_MAIL_TRANSPORT=log` ([email](email.md#try-kaeso-without-a-mail-server)).

curl and the SDKs don't trust Caddy's local authority. Copy its root certificate out of the container:

```bash
docker compose cp \
  caddy:/data/caddy/pki/authorities/local/root.crt caddy-root.crt
```

Then hand the certificate to the client that calls Kaeso:

```bash tab="curl"
curl --cacert caddy-root.crt https://localhost/v1/models \
  -H "Authorization: Bearer $KAESO_API_KEY"
```

```bash tab="Python"
SSL_CERT_FILE=caddy-root.crt python app.py
```

```bash tab="TypeScript"
NODE_EXTRA_CA_CERTS=caddy-root.crt node app.mjs
```

## Change the services

Put your own changes to the services in `compose.override.yaml`, next to `compose.yaml`. Compose merges the two files, and an upgrade replaces only `compose.yaml`, so your changes stay. For example, to log the gateway in more detail:

```yaml title="compose.override.yaml"
services:
  gateway:
    environment:
      KAESO_LOG_LEVEL: info,kaeso_server=debug
```

Apply the change with `docker compose up -d --wait`. Compose recreates only the services whose settings changed. [Configuration](configuration.md) lists every setting of the gateway.

## Choose how to keep request logs

Request logs live in ClickHouse. The bundled ClickHouse in `compose.yaml` is the recommended setup: it needs nothing from you and keeps the logs on the same server. Usage, limits, budgets and budget alerts don't depend on it, because Kaeso keeps usage in PostgreSQL. ClickHouse takes the most memory of the services ([requirements](requirements.md#memory-and-images)), so an install can also use a ClickHouse you already run, or keep no request logs at all.

| Request logs | `KAESO_CLICKHOUSE_URL` in `.env` | Bundled `clickhouse` service |
| --- | --- | --- |
| In the bundled ClickHouse, the default | Not set | Runs |
| In your own ClickHouse | Your ClickHouse's URL | Off |
| Off | Set and empty | Off |

The gateway reads the setting when it starts. To go back to the default, remove both changes below and run `docker compose up -d --wait`.

### Turn request logs off

1. Add `KAESO_CLICKHOUSE_URL=` with an empty value to `.env`. Keep `CLICKHOUSE_PASSWORD` there: Compose checks it even while the bundled ClickHouse is off.
2. Keep Compose from starting the bundled ClickHouse by putting it in a profile you never turn on:

   ```yaml title="compose.override.yaml"
   services:
     clickhouse:
       profiles: [logs]
   ```

3. Start the gateway with the new settings, then remove the ClickHouse container that still runs. Its volume `kaeso_clickhouse` keeps the earlier logs until you remove it.

   ```bash
   docker compose up -d --wait
   docker compose rm -sf clickhouse
   ```

The console then shows no **Request logs**, and the organization's **Logging** page says that request logs are off. The Platform API answers `404` on the logs endpoints, and `/readyz` reports `clickhouse` as `off`. Requests served while logs are off are never logged, even after you turn logs back on.

### Use your own ClickHouse

The gateway needs only ClickHouse's HTTP interface, plain or with TLS, and a user that may create a database and its tables. Put the URL, with that user and password, in `.env`. Percent-encode any character in them other than letters and digits:

```bash
KAESO_CLICKHOUSE_URL=https://kaeso:secret@clickhouse.example.com:8443
```

Then turn off the bundled ClickHouse with the same `compose.override.yaml` and commands as [above](#turn-request-logs-off). The gateway creates the database `KAESO_CLICKHOUSE_DATABASE`, `kaeso` by default, and runs its migrations there. Back up that ClickHouse as you back up the rest of it.

## Run Kaeso's commands

The gateway image is also Kaeso's command line. Run a command in the running gateway container, where it uses the gateway's settings:

```bash
docker compose exec gateway kaeso user reset-2fa owner@example.com
```

| Command | Does |
| --- | --- |
| `kaeso setup-link` | Prints the setup link while the install has no users. The link works once, for 24 hours, and a new one retires the older ones |
| `kaeso user sign-in-link <email>` | Prints a one-time sign-in link for an active user, valid for 15 minutes ([get back in](sign-in.md#get-back-in)) |
| `kaeso user reset-2fa <email>` | Removes a user's two-factor authentication and recovery codes, ends their sessions and emails them. The instance audit log records it |
| `kaeso sso verify-domain <domain>` | Marks an SSO email domain verified without its TXT record, for a domain that public DNS doesn't answer for. `--org <org>` names the organization when several list the domain. The organization's audit log records it |
| `kaeso instance admin-sso <org\|none>` | Chooses the organization whose single sign-on may administer the instance, by an id such as `org_…`, or `none` for none, when no instance admin can change it in the console ([instance admins](sign-in.md#instance-admins)). The organization needs an SSO connection. The instance audit log records it |
| `kaeso master-key generate` | Prints a new master key as `id:base64` |
| `kaeso master-key rewrap` | Re-encrypts stored secrets with the first master key and reports which key ids are still in use ([rotate the master key](backups.md#rotate-the-master-key)) |
| `kaeso migrate` | Runs the PostgreSQL migrations, and the ClickHouse ones while request logs are on, then exits |
| `kaeso serve` | Runs the gateway. It is the image's default command |

## Next steps

- [Configuration](configuration.md): Look up every setting and where it goes.
- [Domains and TLS](domains-and-tls.md): Get a certificate, or run Kaeso behind your own proxy.
- [Backups and master keys](backups.md): Back up the databases and keep the master keys safe.
