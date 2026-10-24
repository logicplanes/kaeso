---
title: Installation
description: Install a particular release, try Kaeso on your computer, change its services and run its commands.
order: 3
---

The [quick start](../get-started/quick-start.md) installs the newest release on a server. This page covers what you might do differently, and the commands you run on an install.

## Install a particular release

The quick start downloads the `compose.yaml` of the newest stable release. To install another release, download the file from that release's tag:

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
| `kaeso migrate` | Runs the PostgreSQL and ClickHouse migrations, then exits |
| `kaeso serve` | Runs the gateway. It is the image's default command |

## Next steps

- [Configuration](configuration.md): Look up every setting and where it goes.
- [Domains and TLS](domains-and-tls.md): Get a certificate, or run Kaeso behind your own proxy.
- [Backups and master keys](backups.md): Back up the databases and keep the master keys safe.
