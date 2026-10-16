---
title: Self-hosting
description: Run, configure and look after a Kaeso install.
order: 2
---

A Kaeso install is a set of containers on one domain. The Compose file attached to each Kaeso release starts all of them; the [quick start](../get-started/quick-start.md) is the fastest way to a running install.

| Service | Image | What it does | Reachable from |
| --- | --- | --- | --- |
| `caddy` | `caddy:2.11.7-alpine` | Terminates TLS for your domain, with a certificate it gets and renews by itself, and routes requests | the internet, ports 80 and 443 |
| `gateway` | `ghcr.io/logicplanes/kaeso-gateway` | Serves the model API (`/v1`) and the Platform API (`/platform`) | Caddy |
| `console` | `ghcr.io/logicplanes/kaeso-console` | Serves the web interface (every other path) | Caddy |
| `postgres` | `postgres:18.6-alpine` | PostgreSQL: users, organizations, keys, encrypted provider keys, catalogues, limits, usage, audit logs | the gateway only |
| `clickhouse` | `clickhouse/clickhouse-server:26.3.48.2-alpine` | ClickHouse: request logs and usage rollups | the gateway only |
| `valkey` | `valkey/valkey:9.1.2-alpine` | Valkey: rate and budget counters and shared caches | the gateway only |

The gateway and console images carry the version of the release whose `compose.yaml` you use. The databases sit on an internal network that nothing outside can reach, and their data lives in named Docker volumes (`postgres`, `clickhouse`, `valkey`, and `caddy_data` for the certificates). The gateway's metrics listener, port 9100, is not routed by Caddy.

## More than one gateway

Gateway instances keep no state of their own. To run several, put them behind the same proxy and point them at the same PostgreSQL, ClickHouse and Valkey: caches are invalidated across instances through Valkey, background jobs run on one instance at a time under PostgreSQL locks, and each instance buffers and writes its own request log rows and usage.

## Pages

| Page | What it covers |
| --- | --- |
| [Requirements](requirements.md) | Server size, disk for request logs, network |
| [Installation](installation.md) | The files of an install, what happens at the first start, local changes, the command line |
| [Configuration](configuration.md) | How settings are read, every `KAESO_*` setting, and the API key throttle |
| [Domains and TLS](domains-and-tls.md) | The domain, certificates, your own reverse proxy, long streams |
| [Email](email.md) | SMTP, what Kaeso sends, languages, testing |
| [Upgrades](upgrades.md) | Moving to a new release |
| [Backups and master keys](backups.md) | What to back up, restoring, rotating the master key |
| [Health and metrics](monitoring.md) | Health checks, Prometheus metrics and logs |
| [Troubleshooting](troubleshooting.md) | Common problems and how to solve them |
