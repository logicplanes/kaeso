---
title: Self-hosting
description: Run, configure and look after a Kaeso install.
order: 2
---

A Kaeso install is six containers behind one domain, started by the `compose.yaml` attached to every release. The [quick start](../get-started/quick-start.md) gets one running; these pages cover the rest.

| Service | Image | Role | Reachable from |
| --- | --- | --- | --- |
| `caddy` | `caddy:2.11.7-alpine` | Terminates TLS with a certificate it obtains and renews itself, and routes requests | The internet, ports 80 and 443 |
| `gateway` | `ghcr.io/logicplanes/kaeso-gateway` | Serves the model API (`/v1`) and the Platform API (`/platform`) | Caddy |
| `console` | `ghcr.io/logicplanes/kaeso-console` | Serves the web interface (every other path) | Caddy |
| `postgres` | `postgres:18.6-alpine` | Users, organizations, keys, encrypted provider keys, catalogues, limits, usage, audit logs | The gateway only |
| `clickhouse` | `clickhouse/clickhouse-server:26.3.48.2-alpine` | Request logs and usage rollups | The gateway only |
| `valkey` | `valkey/valkey:9.1.2-alpine` | Rate and budget counters, shared caches | The gateway only |

The gateway and console images carry the version of the release whose `compose.yaml` you run; the third-party images are pinned by version and digest.

## Isolation

- The databases sit on an internal network with no route outside. Their data lives in the named volumes `postgres`, `clickhouse` and `valkey`; Caddy keeps its certificates in `caddy_data` and `caddy_config`.
- No container can gain privileges. The gateway and the console run without Linux capabilities, and the gateway's file system is read-only. Caddy keeps only the right to bind ports 80 and 443, and sends `Strict-Transport-Security`.
- The gateway signs in to PostgreSQL as `kaeso`, which owns the Kaeso database but is not a superuser. ClickHouse's `kaeso` user can't manage users or grants. Valkey requires its password on every connection.
- Caddy doesn't route the gateway's metrics port, 9100.

## More than one gateway

Gateway instances keep no state of their own. To run several, put them behind the same proxy and point them at the same PostgreSQL, ClickHouse and Valkey. Caches are invalidated across instances through Valkey, background jobs run on one instance at a time under PostgreSQL locks, and each instance buffers and writes its own request log rows and usage.

## Pages

| Page | Covers |
| --- | --- |
| [Requirements](requirements.md) | Server size, network, disk for request logs |
| [Installation](installation.md) | The files of an install, the first start, local changes, the command line |
| [Configuration](configuration.md) | How settings are read, every `KAESO_*` setting, API key throttling |
| [Domains and TLS](domains-and-tls.md) | The domain, certificates, your own reverse proxy, long streams |
| [Email](email.md) | SMTP, what Kaeso sends and in which language, testing |
| [Sign-in and single sign-on](sign-in.md) | Sign-in methods, installs on single sign-on alone, getting back in |
| [Upgrades](upgrades.md) | Moving to a new release |
| [Backups and master keys](backups.md) | What to back up, restoring, rotating the master key |
| [Health and metrics](monitoring.md) | Health checks, Prometheus metrics, logs |
| [Troubleshooting](troubleshooting.md) | Common problems and their fixes |
